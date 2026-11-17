use windows::Win32::{
    Foundation::{BOOL, LPARAM, POINT, RECT},
    Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR},
    UI::{
        HiDpi::{
            GetDpiForMonitor, SetProcessDpiAwareness, SetProcessDpiAwarenessContext,
            SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT,
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            MDT_EFFECTIVE_DPI, PROCESS_PER_MONITOR_DPI_AWARE,
        },
        WindowsAndMessaging::{
            GetPhysicalCursorPos, SetPhysicalCursorPos, SetProcessDPIAware, USER_DEFAULT_SCREEN_DPI,
        },
    },
};

use super::core_error;
use crate::{
    ce,
    errors::{Error, Result},
};

pub fn get_cursor_pos() -> Result<(i32, i32)> {
    let mut pt = POINT::default();
    unsafe { ce!(GetPhysicalCursorPos(&mut pt)) }?;
    Ok((pt.x, pt.y))
}

pub fn set_cursor_pos(x: i32, y: i32) -> Result<()> {
    unsafe { ce!(SetPhysicalCursorPos(x, y)) }
}

#[derive(Debug)]
pub struct MonitorInfo {
    pub handle: HMONITOR,
    pub rect: RECT,
    pub scale: u32,
}

pub struct ScopeDpiAwareness {
    old: DPI_AWARENESS_CONTEXT,
}

impl ScopeDpiAwareness {
    pub fn new(v: DPI_AWARENESS_CONTEXT) -> Self {
        let old = unsafe { SetThreadDpiAwarenessContext(v) };
        ScopeDpiAwareness { old }
    }
}

impl Drop for ScopeDpiAwareness {
    fn drop(&mut self) {
        unsafe { SetThreadDpiAwarenessContext(self.old) };
    }
}

pub fn get_monitor_scale_factor(hm: HMONITOR) -> Result<u32> {
    // GetScaleFactorForMonitor() returns a wrong scale value, which is different from the monitor setting.
    // The right value should be calculated from per-screen dpi.
    // Ref: https://stackoverflow.com/questions/31348823/getscalefactorformonitor-value-doesnt-match-actual-scale-applied
    //      https://learn.microsoft.com/en-us/windows/win32/hidpi/wm-dpichanged

    // use windows::Win32::UI::Shell::GetScaleFactorForMonitor;
    // match unsafe { GetScaleFactorForMonitor(hm) } {
    //     Ok(v) => Ok(v.0 as u32),
    //     Err(e) => Err(core_error(e)),
    // }

    let set_aware = ScopeDpiAwareness::new(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
    let mut dpix: u32 = 0;
    let mut dpiy: u32 = 0;
    unsafe {
        ce!(GetDpiForMonitor(
            hm,
            MDT_EFFECTIVE_DPI,
            &mut dpix,
            &mut dpiy
        ))?;
    }
    drop(set_aware);

    Ok(dpix * 100 / USER_DEFAULT_SCREEN_DPI)
}

pub fn thread_set_dpi_aware() {
    unsafe {
        SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

pub fn process_set_dpi_aware() -> bool {
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).is_ok() {
            return true;
        }
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE).is_ok() {
            return true;
        }
        if SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE).is_ok() {
            return true;
        }
        if SetProcessDpiAwareness(PROCESS_PER_MONITOR_DPI_AWARE).is_ok() {
            return true;
        }
        SetProcessDPIAware().as_bool()
    }
}

pub fn get_all_monitors_info() -> Result<Vec<MonitorInfo>> {
    unsafe extern "system" fn enum_fn(
        hm: HMONITOR,
        _hdc: HDC,
        rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let hms = unsafe { &mut *(lparam.0 as *mut Vec<MonitorInfo>) };
        hms.push(MonitorInfo {
            handle: hm,
            rect: *rect,
            scale: 0,
        });
        BOOL(1)
    }

    let mut hms: Vec<MonitorInfo> = Vec::new();
    match unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_fn),
            LPARAM(&mut hms as *mut Vec<MonitorInfo> as isize),
        )
        .as_bool()
    } {
        true => (),
        false => return Err(Error::WinUnknown("EnumDisplayMonitors")),
    }

    for m in &mut hms {
        m.scale = get_monitor_scale_factor(m.handle)?;
    }

    Ok(hms)
}
