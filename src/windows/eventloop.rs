use super::{
    poller::WinPoller, service::WinPlatformService, winwrap::window::wait_window_messages,
};
use crate::{
    errors::Result,
    message::{GenericDevice, MouseControlReactor},
    processor::{message_poller::MessagePoller, Processor},
    setting::Settings,
};

pub struct WinEventLoop {
    processor: Processor,
    platform: WinPlatformService,
    windows: WinPoller,
    messages: MessagePoller,
}

impl WinEventLoop {
    pub fn new(headless: bool, reactor: MouseControlReactor) -> Self {
        let windows = WinPoller::new(reactor.ui_tx.clone(), reactor.mouse_control_tx.clone());
        Self {
            processor: Processor::new(headless),
            platform: WinPlatformService::new(),
            windows,
            messages: MessagePoller::new(reactor),
        }
    }

    pub fn initialize(&mut self) -> Result<()> {
        self.platform.setup_window()?;
        self.processor.initialize(&mut self.platform)?;
        self.windows.initialize()
    }

    pub fn load_config(&mut self, config: Settings) -> Result<()> {
        self.processor
            .apply_new_settings(config.processor, &mut self.platform)
    }

    pub fn terminate(&mut self) -> Result<()> {
        self.windows.terminate()?;
        self.processor.terminate(&mut self.platform)
    }

    pub fn scan_devices(&mut self) -> Result<Vec<GenericDevice>> {
        self.processor.scan_devices(&mut self.platform)
    }

    pub fn poll_wm_messages(&mut self, max_events: u32, timeout_ms: u32) -> Result<bool> {
        wait_window_messages(timeout_ms);
        self.windows
            .poll(max_events, &mut self.processor, &mut self.platform)
    }

    pub fn poll_messages(&mut self) -> bool {
        self.messages.poll(&mut self.processor, &mut self.platform)
    }
}
