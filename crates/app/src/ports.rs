//! Port selection: which JACK ports the app connects to, decided from the
//! saved choice and the ports that exist right now. Pure, so it is testable
//! without a JACK server.

use serde::{Deserialize, Serialize};

/// The ports the app connects to, by full JACK name. `None` means
/// unconnected.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Routing {
    pub input: Option<String>,
    pub output_1: Option<String>,
    pub output_2: Option<String>,
}

/// One of the app's three connections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Input,
    Output1,
    Output2,
}

impl Slot {
    pub const ALL: [Slot; 3] = [Slot::Input, Slot::Output1, Slot::Output2];
}

impl Routing {
    pub fn get(&self, slot: Slot) -> Option<&str> {
        match slot {
            Slot::Input => self.input.as_deref(),
            Slot::Output1 => self.output_1.as_deref(),
            Slot::Output2 => self.output_2.as_deref(),
        }
    }

    pub fn set(&mut self, slot: Slot, port: Option<String>) {
        match slot {
            Slot::Input => self.input = port,
            Slot::Output1 => self.output_1 = port,
            Slot::Output2 => self.output_2 = port,
        }
    }
}

impl PortLists {
    /// The ports a slot can connect to.
    pub fn for_slot(&self, slot: Slot) -> &[String] {
        match slot {
            Slot::Input => &self.sources,
            Slot::Output1 | Slot::Output2 => &self.sinks,
        }
    }
}

/// The physical audio ports that exist right now, by full JACK name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PortLists {
    /// Where sound comes from (interface inputs; JACK output ports).
    pub sources: Vec<String>,
    /// Where sound goes (interface outputs; JACK input ports).
    pub sinks: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    Connected,
    /// Some slot is unchosen and the interface is not there to fill it.
    InterfaceNotFound,
    /// Chosen ports that no longer exist (e.g. the interface was unplugged).
    Vanished(Vec<String>),
}

/// Name fragment that identifies the bassist's interface.
const INTERFACE_PATTERN: &str = "Scarlett";

impl Routing {
    /// The startup choice: each saved port that exists, otherwise the
    /// interface default for that slot. With neither, the saved port is kept
    /// so it reconnects if it appears later.
    pub fn at_launch(saved: &Routing, ports: &PortLists) -> Routing {
        let default = Routing::interface_default(ports);
        let pick =
            |saved: &Option<String>, available: &[String], default: Option<String>| match saved {
                Some(name) if available.contains(name) => saved.clone(),
                _ => default.or_else(|| saved.clone()),
            };
        Routing {
            input: pick(&saved.input, &ports.sources, default.input),
            output_1: pick(&saved.output_1, &ports.sinks, default.output_1),
            output_2: pick(&saved.output_2, &ports.sinks, default.output_2),
        }
    }

    /// Fills empty slots with the interface default, for when the interface
    /// appears after launch. Returns whether anything changed.
    pub fn adopt_defaults(&mut self, ports: &PortLists) -> bool {
        let default = Routing::interface_default(ports);
        let before = self.clone();
        self.input = self.input.take().or(default.input);
        self.output_1 = self.output_1.take().or(default.output_1);
        self.output_2 = self.output_2.take().or(default.output_2);
        *self != before
    }

    /// Whether every slot points at a port that exists.
    pub fn health(&self, ports: &PortLists) -> Health {
        let slots = [
            (&self.input, &ports.sources),
            (&self.output_1, &ports.sinks),
            (&self.output_2, &ports.sinks),
        ];
        let vanished: Vec<String> = slots
            .iter()
            .filter_map(|(slot, available)| slot.as_ref().filter(|name| !available.contains(name)))
            .cloned()
            .collect();
        if !vanished.is_empty() {
            Health::Vanished(vanished)
        } else if slots.iter().any(|(slot, _)| slot.is_none()) {
            Health::InterfaceNotFound
        } else {
            Health::Connected
        }
    }

    /// Interface input 1 and outputs 1/2, whatever the card profile calls
    /// them (`capture_FL` or `capture_AUX0`, and so on).
    fn interface_default(ports: &PortLists) -> Routing {
        let interface = |names: &[String]| {
            let mut found: Vec<String> = names
                .iter()
                .filter(|name| name.contains(INTERFACE_PATTERN))
                .cloned()
                .collect();
            found.sort();
            found.into_iter()
        };
        let mut sinks = interface(&ports.sinks);
        Routing {
            input: interface(&ports.sources).next(),
            output_1: sinks.next(),
            output_2: sinks.next(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: &str = "Scarlett 4i4 4th Gen Analog Surround 2.1";

    fn scarlett(port: &str) -> String {
        format!("{S}:{port}")
    }

    fn with_scarlett() -> PortLists {
        PortLists {
            sources: vec![
                "Built-in Audio Analog Stereo:capture_FL".into(),
                scarlett("capture_LFE"),
                scarlett("capture_FR"),
                scarlett("capture_FL"),
            ],
            sinks: vec![
                "Built-in Audio Analog Stereo:playback_FL".into(),
                scarlett("playback_LFE"),
                scarlett("playback_FR"),
                scarlett("playback_FL"),
            ],
        }
    }

    #[test]
    fn launch_without_saved_ports_picks_scarlett_input_1_and_outputs_1_2() {
        let routing = Routing::at_launch(&Routing::default(), &with_scarlett());
        assert_eq!(
            routing,
            Routing {
                input: Some(scarlett("capture_FL")),
                output_1: Some(scarlett("playback_FL")),
                output_2: Some(scarlett("playback_FR")),
            }
        );
    }

    #[test]
    fn launch_keeps_saved_ports_that_exist_and_replaces_ones_that_do_not() {
        let saved = Routing {
            input: Some("Built-in Audio Analog Stereo:capture_FL".into()),
            output_1: Some("Gone Device:playback_FL".into()),
            output_2: Some(scarlett("playback_LFE")),
        };
        let routing = Routing::at_launch(&saved, &with_scarlett());
        assert_eq!(
            routing,
            Routing {
                input: Some("Built-in Audio Analog Stereo:capture_FL".into()),
                output_1: Some(scarlett("playback_FL")),
                output_2: Some(scarlett("playback_LFE")),
            }
        );
    }

    #[test]
    fn launch_with_interface_absent_keeps_saved_ports_to_wait_for_them() {
        let saved = Routing {
            input: Some(scarlett("capture_FR")),
            output_1: None,
            output_2: None,
        };
        let only_builtin = PortLists {
            sources: vec!["Built-in Audio Analog Stereo:capture_FL".into()],
            sinks: vec!["Built-in Audio Analog Stereo:playback_FL".into()],
        };
        assert_eq!(Routing::at_launch(&saved, &only_builtin), saved);
    }

    #[test]
    fn interface_plugged_in_later_fills_empty_slots_only() {
        let mut routing = Routing {
            input: Some(scarlett("capture_FR")),
            output_1: None,
            output_2: None,
        };
        assert!(routing.adopt_defaults(&with_scarlett()));
        assert_eq!(
            routing,
            Routing {
                input: Some(scarlett("capture_FR")),
                output_1: Some(scarlett("playback_FL")),
                output_2: Some(scarlett("playback_FR")),
            }
        );
        assert!(!routing.adopt_defaults(&with_scarlett()));
    }

    #[test]
    fn health_reports_connected_missing_interface_and_vanished_ports() {
        let all_present = Routing::at_launch(&Routing::default(), &with_scarlett());
        assert_eq!(all_present.health(&with_scarlett()), Health::Connected);

        assert_eq!(
            Routing::default().health(&PortLists::default()),
            Health::InterfaceNotFound
        );

        // Unplugged: the chosen ports are gone.
        assert_eq!(
            all_present.health(&PortLists::default()),
            Health::Vanished(vec![
                scarlett("capture_FL"),
                scarlett("playback_FL"),
                scarlett("playback_FR"),
            ])
        );
    }
}
