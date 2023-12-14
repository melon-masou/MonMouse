pub mod control;
pub mod errors;
pub mod notify;
pub mod windows;

mod utils;

use std::thread;

use log::debug;

use crate::windows::win_processor::WinEventLoop;

fn setup_logger() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Trace)
        .init();
}

fn main() {
    setup_logger();

    debug!("MonMouse started");

    let mut eventloop = WinEventLoop::new();
    let device_thread = thread::spawn(move || {
        eventloop.run().unwrap();
    });
    device_thread.join().unwrap();

    debug!("MonMouse ended normally")
}
