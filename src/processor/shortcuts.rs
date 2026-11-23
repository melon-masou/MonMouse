use std::collections::BTreeMap;

use log::warn;
use monmouse_lib::plugin::PluginKey;

use super::platform::PlatformService;
use crate::{
    errors::{Error, Result},
    message::ShortcutID,
    setting::MappingRule,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutSource {
    Main,
    Plugin(PluginKey),
}

pub enum ShortcutAction {
    Main(ShortcutID),
    Plugin(PluginKey, String),
    Mapping(MappingRule),
}

impl ShortcutAction {
    fn source(&self) -> ShortcutSource {
        match self {
            Self::Main(_) | Self::Mapping(_) => ShortcutSource::Main,
            Self::Plugin(key, _) => ShortcutSource::Plugin(key),
        }
    }
}

#[derive(Default)]
pub struct ShortcutManager {
    bindings: BTreeMap<i32, (String, ShortcutAction)>,
}

impl ShortcutManager {
    pub fn action(&self, id: i32) -> Option<&ShortcutAction> {
        self.bindings.get(&id).map(|(_, action)| action)
    }

    pub fn register_shortcuts(
        &mut self,
        platform: &mut impl PlatformService,
        source: ShortcutSource,
        bindings: Vec<(String, ShortcutAction)>,
        restore_on_error: bool,
    ) -> Result<()> {
        if let Some(actual) = bindings
            .iter()
            .map(|(_, action)| action.source())
            .find(|&actual| actual != source)
        {
            return Err(Error::ShortcutRegistration(format!(
                "Shortcut source mismatch: expected {source:?}, got {actual:?}"
            )));
        }

        let ids: Vec<_> = self
            .bindings
            .iter()
            .filter_map(|(&id, (_, action))| (action.source() == source).then_some(id))
            .collect();

        let mut errors = Vec::new();
        let mut removed = Vec::new();
        for id in ids {
            match platform.unregister_shortcut(id) {
                Ok(()) => removed.push(self.bindings.remove(&id).unwrap()),
                Err(error) => errors.push(error),
            }
        }
        let mut registered = Vec::new();
        for (text, action) in bindings {
            if text.is_empty() {
                continue;
            }
            match platform.register_shortcut(&text) {
                Ok(id) => {
                    self.bindings.insert(id, (text, action));
                    registered.push(id);
                }
                Err(error) => errors.push(error),
            }
        }
        if errors.is_empty() {
            return Ok(());
        }
        if restore_on_error {
            for id in registered {
                match platform.unregister_shortcut(id) {
                    Ok(()) => {
                        self.bindings.remove(&id);
                    }
                    Err(error) => warn!("Could not clear a shortcut while restoring: {error}"),
                }
            }
            for (text, action) in removed {
                match platform.register_shortcut(&text) {
                    Ok(id) => {
                        self.bindings.insert(id, (text, action));
                    }
                    Err(error) => warn!("Could not restore shortcut: {error}"),
                }
            }
        }
        let details = errors.iter().map(ToString::to_string).collect::<Vec<_>>();
        Err(Error::ShortcutRegistration(details.join("\n")))
    }
}
