use std::collections::HashMap;

use super::{core_error, hstring_from_buffer};
use crate::ce;
use crate::errors::Result;
use crate::input_control::MousePos;

use windows::core::{w, Owned, HSTRING};
use windows::Win32::Foundation::{COLORREF, POINT};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateSolidBrush, DeleteObject, Ellipse, EndPaint, FillRect, GetObjectW,
    InvalidateRect, MonitorFromPoint, Polygon, RectVisible, UpdateWindow, BITMAP, HBRUSH, HMONITOR,
    MONITOR_DEFAULTTONEAREST, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Registry::{
    RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_EXPAND_SZ, RRF_RT_REG_SZ,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetSystemMetricsForDpi, MDT_EFFECTIVE_DPI};

use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DrawIconEx, GetClientRect, GetIconInfo, GetSystemMetrics, GetWindowLongPtrW,
    LoadImageW, PostQuitMessage, RegisterClassW, SetLayeredWindowAttributes, SetWindowLongPtrW,
    SetWindowPos, ShowWindow, DI_NORMAL, GWLP_USERDATA, HCURSOR, HICON, HWND_TOPMOST, ICONINFO,
    IDC_ARROW, IMAGE_CURSOR, LR_LOADFROMFILE, LR_SHARED, LWA_COLORKEY, SM_CXCURSOR,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE,
    SWP_NOZORDER, SW_HIDE, SW_SHOWNOACTIVATE, WM_DESTROY, WM_PAINT, WNDCLASSW, WS_EX_LAYERED,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::Win32::{
    Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM},
    Graphics::Gdi::HDC,
    UI::WindowsAndMessaging::CreateWindowExW,
};

// LWA_COLORKEY transparency color; a rare near-black value to avoid
// blurring or erasing mouse edge pixels.
const TRANSPARENT_COLOR: COLORREF = COLORREF(0x00090807);

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CursorMarker {
    None,
    Inactive,
    Injected,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CursorPos {
    pub position: MousePos,
    pub marker: CursorMarker,
}

struct OverlayContext {
    transparent_brush: HBRUSH,
    cursors_list: Vec<CursorPos>,
    arrow_file: Option<HSTRING>,
    cursor_images: HashMap<i32, Owned<HCURSOR>>,
}

impl OverlayContext {
    fn create() -> Self {
        OverlayContext {
            transparent_brush: unsafe { CreateSolidBrush(TRANSPARENT_COLOR) },
            cursors_list: vec![],
            arrow_file: system_arrow_file(),
            cursor_images: HashMap::new(),
        }
    }
}

impl Drop for OverlayContext {
    fn drop(&mut self) {
        unsafe {
            let _ = DeleteObject(self.transparent_brush);
        }
    }
}

fn cursor_dirty_rect(c: &CursorPos, vx: i32, vy: i32) -> RECT {
    let x = c.position.x - vx;
    let y = c.position.y - vy;
    const PAD: i32 = 128;
    RECT {
        left: x - PAD,
        top: y - PAD,
        right: x + PAD,
        bottom: y + PAD,
    }
}

// Should be called in same thread with overlay window eventloop, as
// a lock-free context is used to save cursors list for rendering.
pub fn trigger_draw_cursors(hwnd: HWND, new_list: Vec<CursorPos>) {
    let ctx = unsafe {
        &mut *match get_overlay_context(hwnd) {
            Some(v) => v,
            None => return,
        }
    };
    if ctx.cursors_list == new_list {
        return;
    }
    let show = !new_list.is_empty() && ctx.cursors_list.is_empty();
    let hide = new_list.is_empty() && !ctx.cursors_list.is_empty();
    unsafe {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        for index in 0..ctx.cursors_list.len().max(new_list.len()) {
            let old = ctx.cursors_list.get(index);
            let new = new_list.get(index);
            if old == new {
                continue;
            }
            for cursor in old.into_iter().chain(new) {
                let dirty = cursor_dirty_rect(cursor, vx, vy);
                let _ = InvalidateRect(hwnd, Some(&dirty), false);
            }
        }
    }
    ctx.cursors_list = new_list;
    if show {
        show_overlay(hwnd);
    }
    if hide {
        hide_overlay(hwnd);
    }
}

fn get_monitor_dpi(hmon: HMONITOR) -> Result<u32> {
    let mut dpi_x: u32 = 0;
    let mut dpi_y: u32 = 0;
    unsafe {
        ce!(GetDpiForMonitor(
            hmon,
            MDT_EFFECTIVE_DPI,
            &mut dpi_x,
            &mut dpi_y
        ))?;
    }
    Ok(dpi_x)
}

fn system_arrow_file() -> Option<HSTRING> {
    let key = w!("Control Panel\\Cursors");
    let value = w!("Arrow");
    let flags = RRF_RT_REG_SZ | RRF_RT_REG_EXPAND_SZ;
    let mut bytes = 0;
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            flags,
            None,
            None,
            Some(&mut bytes),
        )
    }
    .ok()
    .ok()?;
    let mut path = vec![0u16; (bytes as usize).div_ceil(size_of::<u16>())];
    unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key,
            value,
            flags,
            None,
            Some(path.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    }
    .ok()
    .ok()?;
    let path = hstring_from_buffer(&path).ok()?;
    if path.is_empty() {
        return None;
    }
    Some(path)
}

fn load_system_cursor_for_size(
    size: i32,
    arrow_file: &mut Option<HSTRING>,
    cursor_images: &mut HashMap<i32, Owned<HCURSOR>>,
) -> Result<HICON> {
    if let Some(cursor) = cursor_images.get(&size) {
        return Ok(HICON(cursor.0));
    }
    if let Some(file) = arrow_file.as_ref() {
        // The shared IDC_ARROW can return a cached bitmap at a different size.
        match unsafe { LoadImageW(None, file, IMAGE_CURSOR, size, size, LR_LOADFROMFILE) } {
            Ok(image) => {
                cursor_images.insert(size, unsafe { Owned::new(HCURSOR(image.0)) });
                return Ok(HICON(image.0));
            }
            Err(error) => {
                log::warn!("Could not load Windows arrow cursor file: {error}");
                *arrow_file = None;
            }
        }
    }

    let icon = unsafe {
        ce!(LoadImageW(
            None,
            IDC_ARROW,
            IMAGE_CURSOR,
            size,
            size,
            LR_SHARED
        ))?
    };
    Ok(HICON(icon.0))
}

#[allow(dead_code)]
fn debug_icon_info(icon: HICON) {
    unsafe {
        let mut info = ICONINFO::default();

        if GetIconInfo(icon, &mut info).is_err() {
            log::error!("GetIconInfo failed");
            return;
        }

        log::info!("--- ICONINFO ---");
        log::info!("xHotspot: {}", info.xHotspot);
        log::info!("yHotspot: {}", info.yHotspot);

        if !info.hbmMask.0.is_null() {
            let mut bmp = BITMAP::default();
            let ret = GetObjectW(
                info.hbmMask,
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut _ as _),
            );
            if ret != 0 {
                log::info!(
                    "Mask:  {}x{}, bpp={}",
                    bmp.bmWidth,
                    bmp.bmHeight,
                    bmp.bmBitsPixel
                );
            } else {
                log::info!("Mask:  GetObjectW failed");
            }
        }

        if !info.hbmColor.0.is_null() {
            let mut bmp = BITMAP::default();
            let ret = GetObjectW(
                info.hbmColor,
                std::mem::size_of::<BITMAP>() as i32,
                Some(&mut bmp as *mut _ as _),
            );
            if ret != 0 {
                log::info!(
                    "Color: {}x{}, bpp={}",
                    bmp.bmWidth,
                    bmp.bmHeight,
                    bmp.bmBitsPixel
                );
            } else {
                log::info!("Color: GetObjectW failed");
            }
        }
        if !info.hbmMask.0.is_null() {
            let _ = DeleteObject(info.hbmMask);
        }
        if !info.hbmColor.0.is_null() {
            let _ = DeleteObject(info.hbmColor);
        }
    }
}

// Draw cursors using GDI. The rendering is tuned to make the cursor as clear as possible,
// but on high-DPI displays `DrawIconEx` can still produce artifacts such as blurred edges,
// incorrect scaling.
// Modern approaches like Direct2D could avoid these issues. But introducing a full
// D2D rendering path would be overly complex for now, so this is left as future work.
fn draw_cursors(ctx: &mut OverlayContext, hdc: HDC) -> Result<()> {
    let vx = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let vy = unsafe { GetSystemMetrics(SM_YVIRTUALSCREEN) };

    for c in &ctx.cursors_list {
        let position = c.position;
        let x = position.x - vx;
        let y = position.y - vy;

        let hmon = unsafe {
            MonitorFromPoint(
                POINT {
                    x: position.x,
                    y: position.y,
                },
                MONITOR_DEFAULTTONEAREST,
            )
        };
        let dpi = get_monitor_dpi(hmon)?;
        let size = unsafe { GetSystemMetricsForDpi(SM_CXCURSOR, dpi) }; // scaling
        let bounds = RECT {
            left: if c.marker == CursorMarker::Inactive {
                x - size / 4 - size / 8
            } else {
                x
            },
            top: y,
            right: x + size,
            bottom: y + size,
        };
        if !unsafe { RectVisible(hdc, &bounds) }.as_bool() {
            continue;
        }
        let icon = load_system_cursor_for_size(size, &mut ctx.arrow_file, &mut ctx.cursor_images)?;

        unsafe {
            let bg_brush = ctx.transparent_brush;
            ce!(DrawIconEx(
                hdc, x, y, icon, size, size, 0, bg_brush, DI_NORMAL
            ))?;

            match c.marker {
                CursorMarker::None => (),
                CursorMarker::Inactive => {
                    let marker_r = size / 8;
                    let cx = x - (size / 4);
                    let cy = y + (size / 4);
                    let _ = Ellipse(
                        hdc,
                        cx - marker_r,
                        cy - marker_r,
                        cx + marker_r,
                        cy + marker_r,
                    );
                }
                CursorMarker::Injected => {
                    let half_width = (size / 8).max(2);
                    let height = (f64::from(half_width) * 3.0_f64.sqrt()).round() as i32;
                    let center = x + size - 1 - half_width;
                    let _ = Polygon(
                        hdc,
                        &[
                            POINT { x: center, y },
                            POINT {
                                x: center - half_width,
                                y: y + height,
                            },
                            POINT {
                                x: center + half_width,
                                y: y + height,
                            },
                        ],
                    );
                }
            }
        }
    }
    Ok(())
}

fn draw_overlay(hwnd: HWND) -> LRESULT {
    let mut ps = PAINTSTRUCT::default();
    let Some(ctx) = get_overlay_context(hwnd) else {
        return LRESULT(1);
    };

    unsafe {
        let mut rect = RECT::default();
        if GetClientRect(hwnd, &mut rect).is_err() {
            return LRESULT(1);
        }

        let hdc = BeginPaint(hwnd, &mut ps);
        let ctx = &mut *ctx;
        FillRect(hdc, &ps.rcPaint, ctx.transparent_brush);
        if let Err(e) = draw_cursors(ctx, hdc) {
            log::error!("draw cursors in overlay failed: {:?}", e);
        }
        let _ = EndPaint(hwnd, &ps);
    }
    LRESULT(0)
}

extern "system" fn draw_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => draw_overlay(hwnd),
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn get_overlay_context(hwnd: HWND) -> Option<*mut OverlayContext> {
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut OverlayContext;
        if ptr.is_null() {
            return None;
        }
        Some(ptr)
    }
}

fn get_virtual_screen_rect() -> (i32, i32, i32, i32) {
    // Add,minus one to prevent entering Do Not Disturb Mode
    unsafe {
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN) + 1;
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN) + 1;
        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN) - 1;
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN) - 1;
        (vx, vy, vw, vh)
    }
}

pub fn create_overlay_window(module: Option<HMODULE>) -> Result<(HMODULE, HWND)> {
    let hinstance = match module {
        Some(m) => m,
        None => unsafe { ce!(GetModuleHandleW(None))? },
    };
    let class_name = w!("OverlayWindow");

    let wc = WNDCLASSW {
        lpfnWndProc: Some(draw_overlay_wnd_proc),
        hInstance: HINSTANCE(hinstance.0),
        lpszClassName: class_name,
        hbrBackground: unsafe { CreateSolidBrush(TRANSPARENT_COLOR) },
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };

    let hwnd = unsafe {
        let (vx, vy, vw, vh) = get_virtual_screen_rect();
        ce!(CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name,
            w!("MonMouseOverlay"),
            WS_POPUP,
            vx,
            vy,
            vw,
            vh,
            None,
            None,
            hinstance,
            None,
        ))?
    };
    unsafe {
        ce!(SetLayeredWindowAttributes(
            hwnd,
            TRANSPARENT_COLOR,
            0,
            LWA_COLORKEY
        ))?;
    }

    let ctx = Box::new(OverlayContext::create());
    unsafe {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(ctx) as isize);
        let _ = ShowWindow(hwnd, SW_HIDE);
    };

    Ok((hinstance, hwnd))
}

pub fn refresh_overlay_window(hwnd: HWND) -> Result<()> {
    unsafe {
        let (vx, vy, vw, vh) = get_virtual_screen_rect();
        ce!(SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            vx,
            vy,
            vw,
            vh,
            SWP_NOACTIVATE | SWP_NOZORDER,
        ))
    }
}

fn show_overlay(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = UpdateWindow(hwnd);
    }
}

fn hide_overlay(hwnd: HWND) {
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
    }
}
