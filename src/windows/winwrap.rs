use std::fmt;
use std::mem::size_of;

use crate::errors::{Error, Result};
use crate::windows::constants::*;
use crate::windows::wintypes::*;

use windows::Win32::UI::Input::RAWINPUT;
use windows::{
    core::GUID,
    Win32::{
        Devices::{
            DeviceAndDriverInstallation::{
                CM_Get_DevNode_PropertyW, CM_Get_Device_Interface_ListW,
                CM_Get_Device_Interface_List_SizeW, CM_Get_Device_Interface_PropertyW,
                CM_Locate_DevNodeW, CM_GET_DEVICE_INTERFACE_LIST_ALL_DEVICES,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT, CM_LOCATE_DEVNODE_NORMAL, CR_BUFFER_SMALL,
                CR_NO_SUCH_VALUE, CR_SUCCESS,
            },
            HumanInterfaceDevice::{
                HidD_GetHidGuid, HidD_GetManufacturerString, HidD_GetProductString,
                HidD_GetSerialNumberString,
            },
            Properties::{
                DEVPKEY_Device_Class, DEVPKEY_Device_InstanceId, DEVPKEY_Device_Manufacturer,
                DEVPKEY_Device_Parent, DEVPKEY_Device_Service, DEVPKEY_NAME, DEVPROPKEY,
                DEVPROPTYPE, DEVPROP_TYPE_STRING,
            },
        },
        Foundation::{
            CloseHandle, GetLastError, BOOL, BOOLEAN, ERROR_INSUFFICIENT_BUFFER, GENERIC_READ,
            GENERIC_WRITE, HANDLE, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM,
        },
        Graphics::Gdi::{EnumDisplayMonitors, HDC, HMONITOR},
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        System::{LibraryLoader::GetModuleHandleW, SystemInformation::GetTickCount64},
        UI::{
            HiDpi::{
                GetDpiForMonitor, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT,
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE, MDT_EFFECTIVE_DPI,
            },
            Input::{
                GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList,
                RegisterRawInputDevices, HRAWINPUT, RAWINPUTDEVICE, RAWINPUTDEVICELIST,
                RAWINPUTDEVICE_FLAGS, RAWINPUTHEADER, RAW_INPUT_DEVICE_INFO_COMMAND,
                RIDI_DEVICEINFO, RIDI_DEVICENAME, RID_DEVICE_INFO, RID_DEVICE_INFO_HID,
                RID_DEVICE_INFO_MOUSE, RID_DEVICE_INFO_TYPE, RID_INPUT, RIM_TYPEHID,
                RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
            },
            Shell::{DefSubclassProc, SetWindowSubclass},
            WindowsAndMessaging::{
                CallNextHookEx, CreateWindowExW, GetPhysicalCursorPos, SetPhysicalCursorPos,
                SetTimer, SetWindowsHookExA, UnhookWindowsHookEx, HHOOK, HWND_MESSAGE,
                MSLLHOOKSTRUCT, USER_DEFAULT_SCREEN_DPI, WH_MOUSE_LL, WINDOWS_HOOK_ID,
                WINDOW_EX_STYLE, WINDOW_STYLE,
            },
        },
    },
};

#[derive(PartialEq, Eq, Debug)]
pub enum DeviceType {
    MOUSE,
    KEYBOARD,
    HID,
    UNKNOWN,
}

impl DeviceType {
    pub fn from_rid(t: RID_DEVICE_INFO_TYPE) -> Self {
        match t {
            RIM_TYPEMOUSE => DeviceType::MOUSE,
            RIM_TYPEKEYBOARD => DeviceType::KEYBOARD,
            RIM_TYPEHID => DeviceType::HID,
            _ => DeviceType::UNKNOWN,
        }
    }
}

impl fmt::Display for DeviceType {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", format!("{:?}", self).to_lowercase())
    }
}

pub struct RawinputInfo {
    pub rid_info: RID_DEVICE_INFO,
    pub iface: WString,
}

impl RawinputInfo {
    #[inline]
    pub fn typ(&self) -> DeviceType {
        DeviceType::from_rid(self.rid_info.dwType)
    }
    #[inline]
    pub fn get_mouse(&self) -> &RID_DEVICE_INFO_MOUSE {
        assert!(self.typ() == DeviceType::MOUSE);
        unsafe { &self.rid_info.Anonymous.mouse }
    }
    #[inline]
    pub fn get_hid(&self) -> &RID_DEVICE_INFO_HID {
        assert!(self.typ() == DeviceType::HID);
        unsafe { &self.rid_info.Anonymous.hid }
    }
}

pub struct HidDeviceInfo {
    pub serial_number: WString,
    pub manufacturer: WString,
    pub product: WString,
}

pub struct DeviceIfaceInfo {
    pub instance_id: WString,
    pub name: WString,
    pub service: WString,
    pub class: WString,
    pub manufacurer: WString,
}

pub trait MouseLowLevelHook {
    fn on_mouse_ll(action: u32, e: &mut MSLLHOOKSTRUCT) -> bool;
}

pub struct HookWrap {
    id: WINDOWS_HOOK_ID,
    f: extern "system" fn(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT,
}
impl HookWrap {
    extern "system" fn mouse_ll_hook<T: MouseLowLevelHook>(
        ncode: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if ncode < 0 {
            return unsafe { CallNextHookEx(HHOOK(0), ncode, wparam, lparam) };
        }
        let call_next = T::on_mouse_ll(wparam.0 as u32, lparam_ref::<MSLLHOOKSTRUCT>(&lparam));
        if call_next {
            LRESULT(0)
        } else {
            unsafe { CallNextHookEx(HHOOK(0), ncode, wparam, lparam) }
        }
    }

    pub fn mouse_ll<T: MouseLowLevelHook>() -> HookWrap {
        HookWrap {
            id: WH_MOUSE_LL,
            f: HookWrap::mouse_ll_hook::<T>,
        }
    }
}

pub fn set_windows_hook(hook: HookWrap) -> Result<HHOOK> {
    match unsafe { SetWindowsHookExA(hook.id, Some(hook.f), HINSTANCE::default(), 0) } {
        Ok(v) => Ok(v),
        Err(e) => Err(core_error(e)),
    }
}

pub fn unset_windows_hook(hook: HHOOK) -> Result<()> {
    match unsafe { UnhookWindowsHookEx(hook) } {
        Ok(v) => Ok(v),
        Err(e) => Err(core_error(e)),
    }
}

pub fn device_list_all() -> Result<Vec<RAWINPUTDEVICELIST>> {
    let mut cnt: WSize = 0;
    let mut dev_list: Vec<RAWINPUTDEVICELIST> = Vec::new();

    let res = unsafe { GetRawInputDeviceList(None, &mut cnt, wsize_of::<RAWINPUTDEVICELIST>()) };
    if res == u32::MAX {
        return Err(get_last_error());
    }

    loop {
        dev_list.resize(cnt as usize, RAWINPUTDEVICELIST::default());

        let res = unsafe {
            GetRawInputDeviceList(
                Some(wmut_vec(&mut dev_list)),
                &mut cnt,
                wsize_of::<RAWINPUTDEVICELIST>(),
            )
        };
        if res != u32::MAX {
            dev_list.shrink_to(res as usize);
            return Ok(dev_list);
        }

        let e = unsafe { GetLastError().unwrap_err() };
        if e.code() != ERROR_INSUFFICIENT_BUFFER.to_hresult() {
            continue;
        }
    }
}

pub fn get_last_error() -> Error {
    match unsafe { GetLastError().err() } {
        Some(e) => core_error(e),
        None => Error::WinUnknown,
    }
}

pub fn device_get_rawinput_rid_info(handle: HANDLE) -> Result<RID_DEVICE_INFO> {
    let mut dst = RID_DEVICE_INFO::default();
    let mut size = wsize_of_val(&dst);
    let r = unsafe {
        GetRawInputDeviceInfoW(handle, RIDI_DEVICEINFO, Some(wmut_obj(&mut dst)), &mut size)
    };
    if r == u32::MAX {
        return Err(Error::WinPredefineBufSmall(wsize_of_val(&dst), size));
    }
    Ok(dst)
}

pub fn device_get_rawinput_info<T: IBuffer>(
    handle: HANDLE,
    cmd: RAW_INPUT_DEVICE_INFO_COMMAND,
) -> Result<T> {
    let mut size: WSize = 0;
    let r = unsafe { GetRawInputDeviceInfoW(handle, cmd, None, &mut size) };
    if r != 0 {
        return Err(get_last_error());
    }

    let mut buf = T::new(size);
    loop {
        let r = unsafe { GetRawInputDeviceInfoW(handle, cmd, Some(buf.as_mut_data()), &mut size) };
        if r == u32::MAX {
            if size <= buf.capacity() {
                return Err(Error::WinPredefineBufSmall(buf.capacity(), size));
            }
            buf.resize(size);
            continue;
        }
        return Ok(buf);
    }
}

pub fn device_collect_rawinput_infos(dev_handle: HANDLE) -> Result<RawinputInfo> {
    Ok(RawinputInfo {
        rid_info: device_get_rawinput_rid_info(dev_handle)?,
        iface: device_get_rawinput_info::<WString>(dev_handle, RIDI_DEVICENAME)?,
    })
}

pub fn device_get_iface_prop(
    iface: &WString,
    key: DEVPROPKEY,
    typ: DEVPROPTYPE,
) -> Result<Option<WBuffer>> {
    let mut size: WSize = 0;
    let mut mtyp = typ;

    let cr = unsafe {
        CM_Get_Device_Interface_PropertyW(
            iface.as_pcwstr(),
            wptr(&key),
            wmut_ptr(&mut mtyp),
            None,
            &mut size,
            0,
        )
    };
    match cr {
        CR_BUFFER_SMALL | CR_SUCCESS => {
            if mtyp != typ {
                return Err(cr_error(cr));
            }
        }
        CR_NO_SUCH_VALUE => return Ok(None),
        _ => return Err(cr_error(cr)),
    }

    let mut buf = WBuffer::new(size);
    let cr = unsafe {
        CM_Get_Device_Interface_PropertyW(
            iface.as_pcwstr(),
            wptr(&key),
            wmut_ptr(&mut mtyp),
            Some(buf.as_mut_ptr()),
            &mut size,
            0,
        )
    };
    match cr {
        CR_SUCCESS => {
            if mtyp != typ {
                return Err(cr_error(cr));
            }
            Ok(Some(buf))
        }
        _ => Err(cr_error(cr)),
    }
}

pub fn device_get_node_prop(
    devinst: u32,
    key: DEVPROPKEY,
    typ: DEVPROPTYPE,
) -> Result<Option<WBuffer>> {
    let mut size: WSize = 0;
    let mut mtyp = typ;

    let cr = unsafe {
        CM_Get_DevNode_PropertyW(devinst, wptr(&key), wmut_ptr(&mut mtyp), None, &mut size, 0)
    };
    match cr {
        CR_BUFFER_SMALL | CR_SUCCESS => {
            if mtyp != typ {
                return Err(cr_error(cr));
            }
        }
        CR_NO_SUCH_VALUE => return Ok(None),
        _ => return Err(cr_error(cr)),
    }

    let mut buf = WBuffer::new(size);
    let cr = unsafe {
        CM_Get_DevNode_PropertyW(
            devinst,
            wptr(&key),
            wmut_ptr(&mut mtyp),
            Some(buf.as_mut_ptr()),
            &mut size,
            0,
        )
    };
    match cr {
        CR_SUCCESS => {
            if mtyp != typ {
                return Err(cr_error(cr));
            }
            Ok(Some(buf))
        }
        _ => Err(cr_error(cr)),
    }
}

pub fn locate_devnode_handle(instance_id: &WString) -> Result<u32> {
    let mut handle: u32 = 0;
    let cr = unsafe {
        CM_Locate_DevNodeW(
            &mut handle,
            instance_id.as_pcwstr(),
            CM_LOCATE_DEVNODE_NORMAL,
        )
    };
    match cr {
        CR_SUCCESS => Ok(handle),
        _ => Err(cr_error(cr)),
    }
}

pub fn device_get_iface_infos(iface: &WString) -> Result<DeviceIfaceInfo> {
    let instance_id =
        match device_get_iface_prop(iface, DEVPKEY_Device_InstanceId, DEVPROP_TYPE_STRING)? {
            Some(v) => v,
            None => return Err(Error::WinDeviceNoInstanceID(iface.to_string())),
        }
        .to_wstring();
    let devinst = locate_devnode_handle(&instance_id)?;

    let getf = |key| -> WString {
        match device_get_node_prop(devinst, key, DEVPROP_TYPE_STRING) {
            Ok(opt) => match opt {
                Some(v) => v.to_wstring(),
                None => WString::encode_from_str(STR_NO_VALUE),
            },
            Err(_) => WString::encode_from_str(STR_FAIL_GET_DEVICE_PROP),
        }
    };

    Ok(DeviceIfaceInfo {
        instance_id,
        name: getf(DEVPKEY_NAME),
        service: getf(DEVPKEY_Device_Service),
        class: getf(DEVPKEY_Device_Class),
        manufacurer: getf(DEVPKEY_Device_Manufacturer),
    })
}

pub fn device_get_ifaces_list(
    instance_id: &WString,
    class_guid: &GUID,
    present: bool,
) -> Result<Vec<WString>> {
    loop {
        let mut size: WSize = 0;
        let cr = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut size,
                wptr(class_guid),
                instance_id.as_pcwstr(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        match cr {
            CR_SUCCESS => (),
            _ => return Err(cr_error(cr)),
        }

        let mut buf = WString::new(size);
        let pre_flag = if present {
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT
        } else {
            CM_GET_DEVICE_INTERFACE_LIST_ALL_DEVICES
        };
        let cr = unsafe {
            CM_Get_Device_Interface_ListW(
                wptr(class_guid),
                instance_id.as_pcwstr(),
                buf.as_mut_slice(),
                pre_flag,
            )
        };
        match cr {
            CR_SUCCESS => return Ok(buf.split_by_eos()),
            CR_BUFFER_SMALL => {
                continue;
            }
            _ => return Err(cr_error(cr)),
        }
    }
}

pub fn device_get_parents(instance_id: &WString, dep_limit: Option<usize>) -> Result<Vec<WString>> {
    let get_parent = |inst_id: &WString| -> Result<Option<WString>> {
        let handle = locate_devnode_handle(inst_id)?;
        let ret = device_get_node_prop(handle, DEVPKEY_Device_Parent, DEVPROP_TYPE_STRING)?;
        Ok(ret.map(|v| v.to_wstring()))
    };

    let mut ret = Vec::<WString>::new();
    let mut inst = instance_id;
    loop {
        if dep_limit.is_some() && ret.len() >= dep_limit.unwrap() {
            break;
        }
        match get_parent(inst)? {
            Some(v) => {
                ret.push(v);
                inst = &ret.last().unwrap();
            }
            None => break,
        }
    }
    Ok(ret)
}

pub struct ScopeHandle(HANDLE);

impl ScopeHandle {
    fn new(h: HANDLE) -> Self {
        ScopeHandle(h)
    }
    fn get(&self) -> &HANDLE {
        &self.0
    }
}

impl Drop for ScopeHandle {
    fn drop(&mut self) {
        let _ = close_handle(self.0);
    }
}

pub fn close_handle(handle: HANDLE) -> Result<()> {
    match unsafe { CloseHandle(handle) } {
        Ok(_) => Ok(()),
        Err(e) => Err(core_error(e)),
    }
}

pub fn device_open_iface(iface: &WString, metaonly: bool) -> Result<ScopeHandle> {
    let desire_access = if metaonly {
        0
    } else {
        (GENERIC_READ | GENERIC_WRITE).0
    };
    let share_mode = FILE_SHARE_READ | FILE_SHARE_WRITE;

    let result = unsafe {
        CreateFileW(
            iface.as_pcwstr(),
            desire_access,
            share_mode,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            HANDLE(0),
        )
    };

    match result {
        Ok(h) => {
            if h.is_invalid() {
                Err(Error::WinInvalidHandle(h.0))
            } else {
                Ok(ScopeHandle::new(h))
            }
        }
        Err(e) => Err(core_error(e)),
    }
}

pub fn device_get_hid_info(instance_id: &WString, present: bool) -> Result<HidDeviceInfo> {
    let hid_class = unsafe { HidD_GetHidGuid() };
    let ifaces = device_get_ifaces_list(instance_id, &hid_class, present)?;
    let iface = match ifaces.last() {
        Some(v) => v,
        None => return Err(Error::WinDeviceInterfaceListEmpty(instance_id.to_string())),
    };

    let iface_hdl = device_open_iface(iface, true)?;

    let mut data = WString::new(256);
    let result_as_str = |ok: BOOLEAN, buf: &WString| -> WString {
        if ok.as_bool() {
            buf.str_before_null()
        } else {
            WString::encode_from_str(STR_NO_VALUE)
        }
    };

    let r = HidDeviceInfo {
        serial_number: result_as_str(
            unsafe {
                HidD_GetSerialNumberString(*iface_hdl.get(), data.as_mut_data(), data.capacity())
            },
            &data,
        ),
        manufacturer: result_as_str(
            unsafe {
                HidD_GetManufacturerString(*iface_hdl.get(), data.as_mut_data(), data.capacity())
            },
            &data,
        ),
        product: result_as_str(
            unsafe { HidD_GetProductString(*iface_hdl.get(), data.as_mut_data(), data.capacity()) },
            &data,
        ),
    };

    // No need get caps, use us_usage instead
    // let mut prepared_data = device_get_rawinput_info::<WBuffer>(dev_hdl, RIDI_PREPARSEDDATA)?;
    // let pd = PHIDP_PREPARSED_DATA(prepared_data.as_mut_data() as isize);
    // match unsafe { HidP_GetCaps(pd, wmut_ptr(&mut result.caps)) } {
    //     HIDP_STATUS_SUCCESS => (),
    //     v => return Err(ERR_WINDOWS_HIDP_ERROR.with_info(v.0)),
    // }

    Ok(r)
}

pub fn create_dummy_window(module: Option<HMODULE>, lpclass_name: Option<&str>) -> Result<HWND> {
    let hinstance = match module {
        Some(m) => m,
        None => match unsafe { GetModuleHandleW(None) } {
            Ok(v) => v,
            Err(e) => return Err(core_error(e)),
        },
    };
    let class = WString::encode_from_str(lpclass_name.unwrap_or("Message")).as_pcwstr();

    // create message-only window
    let hwnd = unsafe {
        CreateWindowExW(
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
        )
    };
    if hwnd.0 == 0 {
        return Err(get_last_error());
    }
    Ok(hwnd)
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
                wmut_ptr(h) as usize,
            )
        },
        None => unsafe { SetWindowSubclass(hwnd, None, uidsubclass, 0) },
    }
    .as_bool();

    if ok {
        Ok(())
    } else {
        Err(get_last_error())
    }
}

pub fn rawinput_reg(
    hwnd: HWND,
    usage: u16,
    page: u16,
    flags: RAWINPUTDEVICE_FLAGS,
) -> RAWINPUTDEVICE {
    RAWINPUTDEVICE {
        usUsage: usage,
        usUsagePage: page,
        dwFlags: flags,
        hwndTarget: hwnd,
    }
}

pub fn register_rawinput_devices(devs: &[RAWINPUTDEVICE]) -> Result<()> {
    let cbsize = size_of::<RAWINPUTDEVICE>() as u32;
    match unsafe { RegisterRawInputDevices(devs, cbsize) } {
        Ok(_) => Ok(()),
        Err(e) => Err(core_error(e)),
    }
}

pub fn get_rawinput_data(handle: HRAWINPUT, data_buf: &mut WBuffer) -> Result<()> {
    let mut size: u32 = 0;
    let header_size = wsize_of::<RAWINPUTHEADER>();
    let res = unsafe { GetRawInputData(handle, RID_INPUT, None, &mut size, header_size) };
    if res != 0 {
        return Err(get_last_error());
    }

    if data_buf.capacity() < size {
        data_buf.resize(size);
    }

    let res = unsafe {
        GetRawInputData(
            handle,
            RID_INPUT,
            Some(data_buf.as_mut_data()),
            &mut size,
            header_size,
        )
    };
    if res == u32::MAX {
        return Err(get_last_error());
    }
    Ok(())
}

// TickWiden widens a DWORD tick which returned by some 32 API, which will reset to zero every 49.7 days.
// Ref: https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-gettickcount
pub struct TickWiden {
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
    pub fn new() -> Self {
        let mut tw = TickWiden {
            accu_tick: 0,
            last_tick: 0,
        };
        tw.flush();
        tw
    }
    #[inline]
    pub fn flush(&mut self) {
        let t = unsafe { GetTickCount64() };
        self.accu_tick = t / Self::MAX_SHORT_TICK * Self::MAX_SHORT_TICK;
    }
    #[inline]
    pub fn widen(&mut self, t: u32) -> u64 {
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
        0 => Err(get_last_error()),
        _ => Ok(()),
    }
}

pub fn get_cur_tick() -> u64 {
    unsafe { GetTickCount64() }
}

pub fn get_cursor_pos() -> Result<(i32, i32)> {
    let mut pt = POINT::default();
    match unsafe { GetPhysicalCursorPos(&mut pt) } {
        Ok(()) => Ok((pt.x, pt.y)),
        Err(e) => Err(core_error(e)),
    }
}

pub fn set_cursor_pos(x: i32, y: i32) -> Result<()> {
    match unsafe { SetPhysicalCursorPos(x, y) } {
        Ok(()) => Ok(()),
        Err(e) => Err(core_error(e)),
    }
}

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

    // match unsafe { GetScaleFactorForMonitor(hm) } {
    //     Ok(v) => Ok(v.0 as u32),
    //     Err(e) => Err(core_error(e)),
    // }

    let set_aware = ScopeDpiAwareness::new(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE);
    let mut dpix: u32 = 0;
    let mut dpiy: u32 = 0;
    match unsafe { GetDpiForMonitor(hm, MDT_EFFECTIVE_DPI, &mut dpix, &mut dpiy) } {
        Ok(_) => (),
        Err(e) => return Err(core_error(e)),
    };
    drop(set_aware);

    Ok(dpix * 100 / USER_DEFAULT_SCREEN_DPI)
}

pub fn get_all_monitors_info() -> Result<Vec<MonitorInfo>> {
    unsafe extern "system" fn enum_fn(
        hm: HMONITOR,
        _hdc: HDC,
        rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let hms = lparam_ref::<Vec<MonitorInfo>>(&lparam);

        let scale = match get_monitor_scale_factor(hm) {
            Ok(v) => v,
            Err(_) => return BOOL(1),
        };

        hms.push(MonitorInfo {
            handle: hm,
            rect: *rect,
            scale,
        });
        BOOL(1)
    }

    let mut hms: Vec<MonitorInfo> = Vec::new();
    match unsafe {
        EnumDisplayMonitors(HDC(0), None, Some(enum_fn), lparam_from(&mut hms)).as_bool()
    } {
        true => (),
        false => return Err(Error::WinUnknown),
    }
    Ok(hms)
}

pub fn rawinput_to_string(ri: &RAWINPUT) -> String {
    match RID_DEVICE_INFO_TYPE(ri.header.dwType) {
        RIM_TYPEMOUSE => {
            let m = unsafe { &ri.data.mouse };
            format!(
                "{{mouse({}); hdl={}, llast=({},{}), flag={}, extra={}}}",
                ri.header.dwType,
                ri.header.hDevice.0,
                m.lLastX,
                m.lLastY,
                m.usFlags,
                m.ulExtraInformation
            )
        }
        RIM_TYPEHID => {
            let m = unsafe { &ri.data.hid };
            format!(
                "{{hid({}); hdl={}, size={}, count={} }}",
                ri.header.dwType, ri.header.hDevice.0, m.dwSizeHid, m.dwCount
            )
        }
        _ => {
            format!(
                "{{other({}), hdl={}}}",
                ri.header.dwType, ri.header.hDevice.0
            )
        }
    }
}