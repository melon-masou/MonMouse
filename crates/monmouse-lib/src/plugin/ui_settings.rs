use super::PluginUi;

pub struct SettingsPage<T> {
    saved: Option<T>,
    draft: Option<T>,
    dirty: bool,
    saving: Option<T>,
}

impl<T> Default for SettingsPage<T> {
    fn default() -> Self {
        Self {
            saved: None,
            draft: None,
            dirty: false,
            saving: None,
        }
    }
}

impl<T: Clone> SettingsPage<T> {
    pub fn ui(
        &mut self,
        ui: &mut PluginUi<'_>,
        save_label: &str,
        cancel_label: &str,
        mut fields: impl FnMut(&mut PluginUi<'_>, &mut T) -> bool,
    ) -> Option<T> {
        let draft = self.draft.as_mut()?;
        let saving = self.saving.is_some();
        let can_save = self.dirty && !saving;
        let mut save = false;
        ui.toolbar(|ui| {
            save = ui.button(save_label, can_save);
            if ui.button(cancel_label, can_save) {
                ui.request_discard();
            }
        });

        ui.form(!saving, |ui| {
            self.dirty |= fields(ui, draft);
        });
        save.then(|| draft.clone())
    }

    pub fn begin_save(&mut self, settings: T) {
        self.saving = Some(settings);
        self.dirty = false;
    }

    pub fn saving_settings(&self) -> Option<&T> {
        self.saving.as_ref()
    }

    pub fn set_settings(&mut self, settings: &T) {
        self.saved = Some(settings.clone());
        self.draft = Some(settings.clone());
        self.saving = None;
        self.dirty = false;
    }

    pub fn on_saved(&mut self, settings: Option<&T>) {
        if let Some(settings) = settings {
            self.set_settings(settings);
        } else {
            self.saving = None;
            self.dirty = true;
        }
    }

    pub fn has_changes(&self) -> bool {
        self.dirty
    }

    pub fn discard_changes(&mut self) {
        self.draft.clone_from(&self.saved);
        self.dirty = false;
    }
}
