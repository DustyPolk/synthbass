//! The window's connection section: port dropdowns, status line, and buffer
//! size.

use eframe::egui;

use crate::connection::{Connection, Status, View};
use crate::dropdown::{self, Item};
use crate::ports::Slot;

pub fn show(ui: &mut egui::Ui, connection: &Connection) {
    let view = connection.view();

    ui.label("Ports");
    egui::Grid::new("ports").num_columns(2).show(ui, |ui| {
        for slot in Slot::ALL {
            ui.label(slot_label(slot));
            port_dropdown(ui, connection, &view, slot);
            ui.end_row();
        }
    });

    let (ok, message) = status_message(&view.status);
    let color = if ok {
        egui::Color32::LIGHT_GREEN
    } else {
        egui::Color32::LIGHT_RED
    };
    ui.colored_label(color, message);
    if let Some(buffer) = view.buffer {
        let ms = |frames: u32| 1000.0 * frames as f32 / buffer.sample_rate as f32;
        let mut text = format!(
            "Buffer: {} samples at {} Hz ({:.1} ms)",
            buffer.frames,
            buffer.sample_rate,
            ms(buffer.frames)
        );
        if let Some(round_trip) = buffer.round_trip_frames {
            text += &format!(" · round trip ≈ {:.1} ms", ms(round_trip));
        }
        ui.label(text);
    }
    if let Some(notice) = &view.notice {
        ui.colored_label(egui::Color32::YELLOW, notice);
    }
}

fn port_dropdown(ui: &mut egui::Ui, connection: &Connection, view: &View, slot: Slot) {
    let current = view.routing.get(slot);
    let available = view.ports.for_slot(slot);
    let shown = match current {
        None => "(none)".to_string(),
        Some(name) if available.iter().any(|p| p == name) => name.to_string(),
        Some(name) => format!("{name} (missing)"),
    };
    let combo = egui::ComboBox::from_id_salt(slot_label(slot))
        .selected_text(shown)
        .width(320.0);
    let none = Item::Choice("(none)", current.is_none(), None);
    let ports = available
        .iter()
        .map(|port| Item::Choice(port, current == Some(port.as_str()), Some(port)));
    if let Some(port) = dropdown::show(ui, combo, std::iter::once(none).chain(ports)) {
        connection.choose(slot, port.cloned());
    }
}

fn slot_label(slot: Slot) -> &'static str {
    match slot {
        Slot::Input => "Input",
        Slot::Output1 => "Output 1",
        Slot::Output2 => "Output 2",
    }
}

fn status_message(status: &Status) -> (bool, String) {
    match status {
        Status::Starting => (false, "Connecting to the audio server…".into()),
        Status::Connected => (true, "Connected".into()),
        Status::ServerUnreachable(err) => (
            false,
            format!("Audio server not reachable ({err}). Is PipeWire running? Retrying…"),
        ),
        Status::ServerLost => (
            false,
            "The audio server stopped and audio could not be restored. Restart Synthbass.".into(),
        ),
        Status::InterfaceNotFound => (
            false,
            "Scarlett not found. Plug it in, or pick ports above.".into(),
        ),
        Status::Vanished(names) => (
            false,
            format!(
                "Port gone: {}. It reconnects when it comes back, or pick another port.",
                names.join(", ")
            ),
        ),
    }
}
