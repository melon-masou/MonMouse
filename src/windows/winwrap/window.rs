use std::cell::RefCell;

use windows::{
    core::{w, HSTRING, PCWSTR},
    Win32::{
        Foundation::{HMODULE, HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, SystemInformation::GetTickCount64},
        UI::{
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::{
                CreateWindowExW, DispatchMessageW, MessageBoxExW, MsgWaitForMultipleObjects,
                PeekMessageW, SetTimer, TranslateMessage, HWND_DESKTOP, HWND_MESSAGE, MB_TOPMOST,
                MESSAGEBOX_RESULT, MSG, PM_REMOVE, QS_ALLINPUT, WINDOW_EX_STYLE, WINDOW_STYLE,
                WM_QUIT, WS_OVERLAPPEDWINDOW,
            },
        },
    },
};

use super::{core_error, get_last_error};
use crate::{ce, errors::Result};

pub fn create_dummy_window(module: Option<HMODULE>) -> Result<(HMODULE, HWND)> {
    let hinstance = match module {
        Some(m) => m,
        None => unsafe { ce!(GetModuleHandleW(None))? },
    };
    let class = w!("Static");

    let hwnd = unsafe {
        ce!(CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            None,
            WS_OVERLAPPEDWINDOW,
            0,
            0,
            0,
            0,
            HWND_DESKTOP,
            None,
            hinstance,
            None,
        ))?
    };
    Ok((hinstance, hwnd))
}

pub fn create_message_only_window(module: Option<HMODULE>) -> Result<(HMODULE, HWND)> {
    let hinstance = match module {
        Some(m) => m,
        None => unsafe { ce!(GetModuleHandleW(None))? },
    };
    let class = w!("Message");

    // create message-only window
    let hwnd = unsafe {
        ce!(CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            None,
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            hinstance,
            None,
        ))?
    };
    Ok((hinstance, hwnd))
}

pub trait SubclassHandler {
    fn subclass_callback(&mut self, umsg: u32, wp: WPARAM, lp: LPARAM, uidsubclass: usize) -> bool;
}

unsafe extern "system" fn subclass_proc<T: SubclassHandler>(
    hwnd: HWND,
    umsg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    uidsubclass: usize,
    dwrefdata: usize,
) -> LRESULT {
    let dp_ptr = dwrefdata as *mut T;
    let dp = dp_ptr.as_mut().unwrap();

    let call_next = dp.subclass_callback(umsg, wparam, lparam, uidsubclass);
    if call_next {
        DefSubclassProc(hwnd, umsg, wparam, lparam)
    } else {
        LRESULT(0)
    }
}

pub fn set_subclass<T: SubclassHandler>(
    hwnd: HWND,
    uidsubclass: usize,
    handler: Option<&mut T>,
) -> Result<()> {
    let ok = match handler {
        Some(h) => unsafe {
            SetWindowSubclass(
                hwnd,
                Some(subclass_proc::<T>),
                uidsubclass,
                h as *mut T as usize,
            )
        },
        None => unsafe { SetWindowSubclass(hwnd, None, uidsubclass, 0) },
    }
    .as_bool();

    if ok {
        Ok(())
    } else {
        Err(get_last_error("SetWindowSubclass"))
    }
}

// TickWiden widens a DWORD tick which returned by some 32 API, which will reset to zero every 49.7 days.
// Ref: https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount
struct TickWiden {
    accu_tick: u64,
    last_tick: u32,
}

impl Default for TickWiden {
    fn default() -> Self {
        Self::new()
    }
}

impl TickWiden {
    const MIN_FLUSH_TICK: u32 = 1000;
    const MAX_SHORT_TICK: u64 = u32::MAX as u64;
    fn new() -> Self {
        let mut tw = TickWiden {
            accu_tick: 0,
            last_tick: 0,
        };
        tw.flush();
        tw
    }
    #[inline]
    fn flush(&mut self) {
        let t = unsafe { GetTickCount64() };
        self.accu_tick = t / Self::MAX_SHORT_TICK * Self::MAX_SHORT_TICK;
    }
    #[inline]
    fn widen(&mut self, t: u32) -> u64 {
        if t >= self.last_tick {
            if t - self.last_tick > Self::MIN_FLUSH_TICK {
                self.last_tick = t;
            }
        } else {
            self.flush();
            self.last_tick = t;
        }
        self.accu_tick + t as u64
    }
}

thread_local! {
    // Hooks and Raw Input run on the registration thread. Normalize both before
    // delivery without putting native clock state into the shared Processor API.
    static INPUT_TICKS: RefCell<TickWiden> = RefCell::new(TickWiden::new());
}

pub(in crate::windows) fn widen_tick(tick: u32) -> u64 {
    INPUT_TICKS.with_borrow_mut(|ticks| ticks.widen(tick))
}

pub trait TimerCallback {
    fn on_timer(hwnd: HWND, msg: u32, nid: usize, time: u32);
}

pub fn set_timer<T: TimerCallback>(hwnd: HWND, nid: usize, elapse_ms: u32) -> Result<()> {
    unsafe extern "system" fn timer_proc<T: TimerCallback>(
        hwnd: HWND,
        msg: u32,
        nid: usize,
        time: u32,
    ) {
        T::on_timer(hwnd, msg, nid, time)
    }

    let res = unsafe { SetTimer(hwnd, nid, elapse_ms, Some(timer_proc::<T>)) };
    match res {
        0 => Err(get_last_error("SetTimer")),
        _ => Ok(()),
    }
}

pub fn get_cur_tick() -> u64 {
    unsafe { GetTickCount64() }
}

pub fn popup_message_box(caption: PCWSTR, text: &HSTRING) -> Result<MESSAGEBOX_RESULT> {
    let ret = unsafe { MessageBoxExW(None, text, caption, MB_TOPMOST, 0) };
    if ret.0 == 0 {
        Err(get_last_error("MessageBoxExW"))
    } else {
        Ok(ret)
    }
}

// Keep dispatch on the owning thread and process each message before requesting the next.
pub fn poll_window_messages(mut max_events: u32, mut handle: impl FnMut(&MSG)) -> bool {
    let mut msg = MSG::default();
    unsafe {
        while max_events > 0 && PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
            if msg.message == WM_QUIT {
                return false;
            }
            handle(&msg);
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
            max_events -= 1;
        }
    }
    true
}

pub fn wait_window_messages(timeout_ms: u32) {
    unsafe {
        MsgWaitForMultipleObjects(None, false, timeout_ms, QS_ALLINPUT);
    }
}
