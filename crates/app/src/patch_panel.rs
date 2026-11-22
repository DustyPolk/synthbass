//! The window's patch section: picker, save, save as, delete, and the
//! unsaved-changes marker.

use eframe::egui;

use crate::patch::Patch;
use crate::patches::PatchLibrary;
use crate::settings;

pub struct PatchPanel {
    library: PatchLibrary,
    /// The patch as last loaded or saved; `None` after deleting it.
    loaded: Option<Patch>,
    settings: settings::Store,
    /// The outcome of the last action, or why a patch could not be found.
    message: Option<String>,
    /// The name being typed while "Save as" is open.
    save_as: Option<String>,
    confirm_delete: bool,
}

impl PatchPanel {
    /// Picks the startup patch: the last one used, else the built-in
    /// default. Returns it for the caller to play.
    pub fn new(library: PatchLibrary, settings: settings::Store) -> (PatchPanel, Patch) {
        let last = settings.get().last_patch;
        let found = last.as_deref().and_then(|name| library.find(name));
        let message = match (&last, found) {
            (Some(name), None) => Some(format!("Patch \"{name}\" is gone; loaded the default.")),
            _ => None,
        };
        let patch = found.unwrap_or(library.default_patch()).clone();
        let panel = PatchPanel {
            library,
            loaded: Some(patch.clone()),
            settings,
            message,
            save_as: None,
            confirm_delete: false,
        };
        (panel, patch)
    }

    /// Draws the section. Returns a patch the user picked, which the caller
    /// plays and makes `current`.
    pub fn show(&mut self, ui: &mut egui::Ui, current: &mut Patch) -> Option<Patch> {
        let mut picked = None;
        let mut focus_name = false;
        ui.horizontal(|ui| {
            let selected = self.loaded.as_ref().map_or("(none)", |p| p.name.as_str());
            egui::ComboBox::from_label("Patch")
                .selected_text(selected)
                .width(200.0)
                .show_ui(ui, |ui| {
                    for patch in self.library.built_in() {
                        if ui.selectable_label(false, &patch.name).clicked() {
                            picked = Some(patch.clone());
                        }
                    }
                    if self.library.user().next().is_some() {
                        ui.separator();
                    }
                    for patch in self.library.user() {
                        if ui.selectable_label(false, &patch.name).clicked() {
                            picked = Some(patch.clone());
                        }
                    }
                });
            if self.loaded.as_ref() != Some(current) {
                ui.colored_label(egui::Color32::YELLOW, "• unsaved changes");
            }
        });

        let user_patch = self
            .loaded
            .as_ref()
            .filter(|p| !self.library.is_built_in(&p.name))
            .map(|p| p.name.clone());
        ui.horizontal(|ui| {
            let save = ui
                .add_enabled(user_patch.is_some(), egui::Button::new("Save"))
                .on_disabled_hover_text("Built-in patches can't be changed; use Save as.");
            if save.clicked()
                && let Some(name) = &user_patch
            {
                self.save(current, name.clone());
            }
            if ui.button("Save as…").clicked() {
                self.save_as = Some(String::new());
                self.confirm_delete = false;
                focus_name = true;
            }
            if ui
                .add_enabled(user_patch.is_some(), egui::Button::new("Delete"))
                .on_disabled_hover_text("Built-in patches can't be deleted.")
                .clicked()
            {
                self.confirm_delete = true;
                self.save_as = None;
            }
        });

        if self.confirm_delete
            && let Some(name) = &user_patch
        {
            ui.horizontal(|ui| {
                ui.label(format!("Delete \"{name}\"?"));
                if ui.button("Delete").clicked() {
                    self.delete(name.clone());
                }
                if ui.button("Cancel").clicked() {
                    self.confirm_delete = false;
                }
            });
        }

        if let Some(typed) = &mut self.save_as {
            let mut save = None;
            let mut cancel = false;
            ui.horizontal(|ui| {
                let field = ui.add(
                    egui::TextEdit::singleline(typed)
                        .hint_text("Patch name")
                        .desired_width(160.0),
                );
                if focus_name {
                    field.request_focus();
                }
                let name = typed.trim().to_string();
                let built_in = self.library.is_built_in(&name);
                let exists = self.library.user().any(|p| p.name == name);
                let label = if exists { "Overwrite" } else { "Create" };
                let enter = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
                let ok = ui
                    .add_enabled(!name.is_empty() && !built_in, egui::Button::new(label))
                    .on_disabled_hover_text("Needs a name that isn't a built-in patch's.");
                if (ok.clicked() || enter) && !name.is_empty() && !built_in {
                    save = Some(name);
                }
                if ui.button("Cancel").clicked() || ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    cancel = true;
                }
            });
            if cancel {
                self.save_as = None;
            } else if let Some(name) = save {
                self.save(current, name);
            }
        }

        for warning in self.library.warnings() {
            ui.colored_label(egui::Color32::LIGHT_RED, warning);
        }
        if let Some(message) = &self.message {
            ui.label(message);
        }

        if let Some(patch) = &picked {
            self.loaded = Some(patch.clone());
            self.message = None;
            self.save_as = None;
            self.confirm_delete = false;
            self.remember(&patch.name);
        }
        picked
    }

    fn save(&mut self, current: &mut Patch, name: String) {
        let mut patch = current.clone();
        patch.name = name;
        match self.library.save(&patch) {
            Ok(()) => {
                self.message = Some(format!("Saved \"{}\".", patch.name));
                self.remember(&patch.name);
                *current = patch.clone();
                self.loaded = Some(patch);
                self.save_as = None;
            }
            Err(err) => self.message = Some(format!("Could not save: {err}.")),
        }
    }

    fn delete(&mut self, name: String) {
        self.confirm_delete = false;
        match self.library.delete(&name) {
            // The sound keeps playing; it is now unsaved until saved again.
            Ok(()) => {
                self.loaded = None;
                self.message = Some(format!("Deleted \"{name}\"."));
                // Otherwise the next launch warns that the patch is gone.
                let forget = |s: &mut settings::Settings| {
                    if s.last_patch.as_deref() == Some(name.as_str()) {
                        s.last_patch = None;
                    }
                };
                if let Err(err) = self.settings.update(forget) {
                    self.message = Some(format!("Could not save settings: {err}"));
                }
            }
            Err(err) => self.message = Some(format!("Could not delete: {err}.")),
        }
    }

    fn remember(&mut self, name: &str) {
        let name = name.to_string();
        if let Err(err) = self.settings.update(|s| s.last_patch = Some(name)) {
            self.message = Some(format!("Could not save settings: {err}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "synthbass-panel-test-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    /// A panel over a fresh patch library and settings file holding `mine`,
    /// with `last_patch` set to `last`.
    fn panel_with(dir: &std::path::Path, last: &str) -> (PatchPanel, settings::Store) {
        let mut library = PatchLibrary::open(Some(dir.join("patches")));
        library.save(&Patch::new("Mine")).unwrap();
        library.save(&Patch::new("Other")).unwrap();
        let (settings, _) = settings::Store::open(Some(dir.join("settings.toml")));
        settings
            .update(|s| s.last_patch = Some(last.into()))
            .unwrap();
        let (panel, _) = PatchPanel::new(library, settings.clone());
        (panel, settings)
    }

    #[test]
    fn deleting_the_remembered_patch_forgets_it_so_the_next_launch_does_not_warn() {
        let dir = temp_dir("delete-last");
        let (mut panel, settings) = panel_with(&dir, "Mine");
        panel.delete("Mine".into());
        assert_eq!(settings.get().last_patch, None);

        let library = PatchLibrary::open(Some(dir.join("patches")));
        let (relaunched, _) = PatchPanel::new(library, settings);
        assert_eq!(relaunched.message, None);
    }

    #[test]
    fn deleting_another_patch_keeps_the_remembered_one() {
        let dir = temp_dir("delete-other");
        let (mut panel, settings) = panel_with(&dir, "Mine");
        panel.delete("Other".into());
        assert_eq!(settings.get().last_patch.as_deref(), Some("Mine"));
    }
}
