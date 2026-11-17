use super::{PlatformService, Processor};
use crate::message::MouseControlReactor;

// Hook input shares this queue with UI commands; preserve their delivery order.
pub(crate) struct MessagePoller {
    reactor: MouseControlReactor,
}

impl MessagePoller {
    pub(crate) fn new(reactor: MouseControlReactor) -> Self {
        Self { reactor }
    }

    pub(crate) fn poll(
        &mut self,
        processor: &mut Processor,
        platform: &mut impl PlatformService,
    ) -> bool {
        while let Some(message) = self.reactor.mouse_control_rx.try_recv() {
            if processor.on_message(message, platform, &self.reactor) {
                return true;
            }
        }
        false
    }
}
