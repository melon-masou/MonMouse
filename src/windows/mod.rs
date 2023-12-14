pub mod constants;
pub mod win_processor;
pub mod wintypes;
pub mod winwrap;

use std::thread;

use self::win_processor::WinEventLoop;

pub fn run_eventloop() {
    let mut eventloop = WinEventLoop::new();
    let device_thread = thread::spawn(move || {
        eventloop.run().unwrap();
    });
    device_thread.join().unwrap();
}
