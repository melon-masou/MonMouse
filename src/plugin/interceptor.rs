use std::fmt;

use monmouse_lib::{
    input::SysMouseEvent,
    plugin::{
        InterceptorMappingHandler, InterceptorMessageHandler, MouseEventFilter, PluginInterceptor,
        PluginKey, PluginMessage,
    },
};

use super::backend::ReactorContextHost;

pub struct RegisteredInterceptor {
    pub context: ReactorContextHost,
    pub handler: PluginInterceptor,
}

impl fmt::Debug for RegisteredInterceptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RegisteredInterceptor")
            .field("key", &self.context.router.key)
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
pub struct InterceptorRegistry {
    message_handlers: Vec<(ReactorContextHost, Box<InterceptorMessageHandler>)>,
    mapping_handlers: Vec<(ReactorContextHost, Box<InterceptorMappingHandler>)>,
    mouse_filters: Vec<(ReactorContextHost, Box<MouseEventFilter>)>,
}

impl InterceptorRegistry {
    pub fn set(&mut self, key: PluginKey, interceptor: Option<Box<RegisteredInterceptor>>) {
        self.message_handlers
            .retain(|(context, _)| context.router.key != key);
        self.mapping_handlers
            .retain(|(context, _)| context.router.key != key);
        self.mouse_filters
            .retain(|(context, _)| context.router.key != key);

        let Some(interceptor) = interceptor else {
            return;
        };
        let RegisteredInterceptor { context, handler } = *interceptor;
        if let Some(on_event) = handler.on_event {
            self.message_handlers.push((context.clone(), on_event));
        }
        if let Some(on_mapping) = handler.on_mapping {
            self.mapping_handlers.push((context.clone(), on_mapping));
        }
        if let Some(filter) = handler.block_mouse_event {
            self.mouse_filters.push((context, filter));
            self.mouse_filters
                .sort_unstable_by_key(|(context, _)| context.router.key);
        }
    }

    pub fn on_event(&mut self, key: PluginKey, message: PluginMessage) {
        if let Some((context, handler)) = self
            .message_handlers
            .iter_mut()
            .find(|(context, _)| context.router.key == key)
        {
            handler(message, context);
        }
    }

    pub fn on_mapping(&mut self, key: &str, callback: &str, to: &str) -> bool {
        let Some((context, handler)) = self
            .mapping_handlers
            .iter_mut()
            .find(|(context, _)| context.router.key == key)
        else {
            return false;
        };
        handler(callback, to, context);
        true
    }

    pub fn blocks_mouse_event(&mut self, event: &SysMouseEvent) -> bool {
        let mut blocked = false;
        for (context, filter) in &mut self.mouse_filters {
            blocked |= filter(event, context);
        }
        blocked
    }
}
