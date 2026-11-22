//! Port selection: which JACK ports the app connects to, decided from the
//! saved choice and the ports that exist right now. Pure, so it is testable
//! without a JACK server.

use serde::{Deserialize, Serialize};

/// The ports the app connects to, by full JACK name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Routing {
    pub input: Choice,
    pub output_1: Choice,
    pub output_2: Choice,
}

/// What one slot connects to.
///
/// In the settings file a port is its name, `Off` is an empty string, and
/// `Auto` is no entry at all, so files from before `Off` existed still load.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "Option<String>", into = "Option<String>")]
pub enum Choice {
    /// Never chosen: takes the interface default whenever there is one.
    #[default]
    Auto,
    /// Deliberately unconnected: no default ever fills it.
    Off,
    Port(String),
}

impl Choice {
    fn port(&self) -> Option<&str> {
        match self {
            Choice::Port(name) => Some(name),
            Choice::Auto | Choice::Off => None,
        }
    }
}

impl From<Option<String>> for Choice {
    fn from(saved: Option<String>) -> Choice {
        match saved {
            None => Choice::Auto,
            Some(name) if name.is_empty() => Choice::Off,
            Some(name) => Choice::Port(name),
        }
    }
}

impl From<Choice> for Option<String> {
    fn from(choice: Choice) -> Option<String> {
        match choice {
            Choice::Auto => None,
            Choice::Off => Some(String::new()),
            Choice::Port(name) => Some(name),
        }
    }
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
    /// The port a slot connects to, if any.
    pub fn get(&self, slot: Slot) -> Option<&str> {
        self.choice(slot).port()
    }

    fn choice(&self, slot: Slot) -> &Choice {
        match slot {
            Slot::Input => &self.input,
            Slot::Output1 => &self.output_1,
            Slot::Output2 => &self.output_2,
        }
    }

    /// Records the user's pick for a slot; `None` leaves it unconnected on
    /// purpose.
    pub fn set(&mut self, slot: Slot, port: Option<String>) {
        let choice = port.map_or(Choice::Off, Choice::Port);
        match slot {
            Slot::Input => self.input = choice,
            Slot::Output1 => self.output_1 = choice,
            Slot::Output2 => self.output_2 = choice,
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
    /// Some slot was never chosen and the interface is not there to fill it.
    InterfaceNotFound,
    /// Chosen ports that no longer exist (e.g. the interface was unplugged).
    Vanished(Vec<String>),
}

/// Name fragment that identifies the bassist's interface.
const INTERFACE_PATTERN: &str = "Scarlett";

impl Routing {
    /// The startup choice: each saved port that exists, otherwise the
    /// interface default for that slot. With neither, the saved port is kept
    /// so it reconnects if it appears later. A slot saved as `Off` stays off.
    pub fn at_launch(saved: &Routing, ports: &PortLists) -> Routing {
        let default = Routing::interface_default(ports);
        let pick = |saved: &Choice, available: &[String], default: Choice| match saved {
            Choice::Off => Choice::Off,
            Choice::Port(name) if available.contains(name) => saved.clone(),
            Choice::Port(_) if default == Choice::Auto => saved.clone(),
            Choice::Port(_) | Choice::Auto => default,
        };
        Routing {
            input: pick(&saved.input, &ports.sources, default.input),
            output_1: pick(&saved.output_1, &ports.sinks, default.output_1),
            output_2: pick(&saved.output_2, &ports.sinks, default.output_2),
        }
    }

    /// Fills `Auto` slots with the interface default, for when the interface
    /// appears after launch. Returns whether anything changed.
    pub fn adopt_defaults(&mut self, ports: &PortLists) -> bool {
        let default = Routing::interface_default(ports);
        let before = self.clone();
        for (slot, default) in [
            (&mut self.input, default.input),
            (&mut self.output_1, default.output_1),
            (&mut self.output_2, default.output_2),
        ] {
            if *slot == Choice::Auto {
                *slot = default;
            }
        }
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
            .filter_map(|(choice, available)| {
                choice
                    .port()
                    .filter(|name| !available.iter().any(|a| a == name))
            })
            .map(str::to_string)
            .collect();
        if !vanished.is_empty() {
            Health::Vanished(vanished)
        } else if slots.iter().any(|(choice, _)| **choice == Choice::Auto) {
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
        let choice = |port: Option<String>| port.map_or(Choice::Auto, Choice::Port);
        Routing {
            input: choice(interface(&ports.sources).next()),
            output_1: choice(sinks.next()),
            output_2: choice(sinks.next()),
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
                input: Choice::Port(scarlett("capture_FL")),
                output_1: Choice::Port(scarlett("playback_FL")),
                output_2: Choice::Port(scarlett("playback_FR")),
            }
        );
    }

    #[test]
    fn launch_keeps_saved_ports_that_exist_and_replaces_ones_that_do_not() {
        let saved = Routing {
            input: Choice::Port("Built-in Audio Analog Stereo:capture_FL".into()),
            output_1: Choice::Port("Gone Device:playback_FL".into()),
            output_2: Choice::Port(scarlett("playback_LFE")),
        };
        let routing = Routing::at_launch(&saved, &with_scarlett());
        assert_eq!(
            routing,
            Routing {
                input: Choice::Port("Built-in Audio Analog Stereo:capture_FL".into()),
                output_1: Choice::Port(scarlett("playback_FL")),
                output_2: Choice::Port(scarlett("playback_LFE")),
            }
        );
    }

    #[test]
    fn launch_with_interface_absent_keeps_saved_ports_to_wait_for_them() {
        let saved = Routing {
            input: Choice::Port(scarlett("capture_FR")),
            output_1: Choice::Auto,
            output_2: Choice::Auto,
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
            input: Choice::Port(scarlett("capture_FR")),
            output_1: Choice::Auto,
            output_2: Choice::Auto,
        };
        assert!(routing.adopt_defaults(&with_scarlett()));
        assert_eq!(
            routing,
            Routing {
                input: Choice::Port(scarlett("capture_FR")),
                output_1: Choice::Port(scarlett("playback_FL")),
                output_2: Choice::Port(scarlett("playback_FR")),
            }
        );
        assert!(!routing.adopt_defaults(&with_scarlett()));
    }

    #[test]
    fn a_slot_chosen_as_none_stays_unconnected_at_launch_and_when_the_interface_appears() {
        let saved = Routing {
            output_2: Choice::Off,
            ..Routing::default()
        };
        let routing = Routing::at_launch(&saved, &with_scarlett());
        assert_eq!(routing.output_2, Choice::Off);
        assert_eq!(routing.get(Slot::Output2), None);

        let mut routing = Routing::at_launch(&saved, &PortLists::default());
        routing.adopt_defaults(&with_scarlett());
        assert_eq!(
            routing,
            Routing {
                input: Choice::Port(scarlett("capture_FL")),
                output_1: Choice::Port(scarlett("playback_FL")),
                output_2: Choice::Off,
            }
        );
    }

    #[test]
    fn choosing_none_is_kept_by_later_passes_over_the_same_ports() {
        let mut routing = Routing::at_launch(&Routing::default(), &with_scarlett());
        routing.set(Slot::Input, None);
        assert!(!routing.adopt_defaults(&with_scarlett()));
        assert_eq!(routing.get(Slot::Input), None);
        // Leaving a slot unconnected on purpose is not a missing interface.
        assert_eq!(routing.health(&with_scarlett()), Health::Connected);
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
