use std::{
    cell::RefCell,
    path::{Path, PathBuf},
};

use monmouse_lib::{
    input::InputInjection,
    plugin::{MainContext, PluginMessage, PluginTarget, ReactorContext, SendTargetDefinition},
};

use crate::{
    message::Message,
    setting::{load_plugin_settings, save_plugin_settings, SendTarget, SendTargetOption},
};

use super::PluginRouter;

pub struct MainContextHost {
    pub router: PluginRouter,
    pub config_dir: Option<PathBuf>,
    pub send_targets: RefCell<Vec<SendTargetOption>>,
}

impl MainContext for MainContextHost {
    fn send(&self, target: PluginTarget, message: PluginMessage) -> bool {
        self.router.send(target, message)
    }

    fn register_shortcuts(&self, bindings: Vec<(String, String)>) -> Result<(), String> {
        let queued = self
            .router
            .bus
            .mouse_control
            .send(Message::RegisterPluginShortcuts {
                key: self.router.key,
                bindings,
            });
        if queued {
            Ok(())
        } else {
            Err("Processor stopped".into())
        }
    }

    fn register_send_targets(&self, targets: Vec<SendTargetDefinition>) {
        self.send_targets.replace(
            targets
                .into_iter()
                .map(|definition| SendTargetOption {
                    target: SendTarget {
                        plugin: self.router.key.to_owned(),
                        callback: definition.callback,
                    },
                    name: definition.name,
                })
                .collect(),
        );
    }

    fn config_dir(&self) -> Option<&Path> {
        self.config_dir.as_deref()
    }

    fn load_settings_yaml(&self) -> Result<Option<String>, String> {
        load_plugin_settings(self.config_dir(), self.router.key)
    }

    fn save_settings_yaml(&self, yaml: &str) -> Result<(), String> {
        save_plugin_settings(self.config_dir(), self.router.key, yaml)
    }

    fn report_settings_saved(&self) {
        self.router.bus.ui.send(Message::SettingsSaved);
    }

    fn report_error(&self, text: &str, alert: bool) {
        self.router.bus.ui.send(Message::PluginError {
            text: text.to_owned(),
            alert,
        });
    }
}

#[derive(Clone)]
pub struct ReactorContextHost {
    pub router: PluginRouter,
    pub inject_inputs: fn(&[InputInjection], usize) -> crate::errors::Result<()>,
}

impl ReactorContext for ReactorContextHost {
    fn send(&self, target: PluginTarget, message: PluginMessage) -> bool {
        self.router.send(target, message)
    }

    fn inject_input(&self, input: InputInjection, extra_info: usize) -> Result<(), String> {
        (self.inject_inputs)(std::slice::from_ref(&input), extra_info)
            .map_err(|error| error.to_string())
    }
}
