use std::{
    fmt::{self, Display},
    mem::{size_of, size_of_val},
};

use windows::{
    core::{GUID, HSTRING},
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
                DEVPROP_TYPE_STRING,
            },
        },
        Foundation::{
            GetLastError, BOOLEAN, ERROR_INSUFFICIENT_BUFFER, GENERIC_READ, GENERIC_WRITE, HANDLE,
        },
        Storage::FileSystem::{
            CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
        },
        UI::Input::{
            GetRawInputDeviceInfoW, GetRawInputDeviceList, RAWINPUTDEVICELIST, RIDI_DEVICEINFO,
            RIDI_DEVICENAME, RID_DEVICE_INFO, RID_DEVICE_INFO_HID, RID_DEVICE_INFO_MOUSE,
            RID_DEVICE_INFO_TYPE, RIM_TYPEHID, RIM_TYPEKEYBOARD, RIM_TYPEMOUSE,
        },
    },
};

use super::{
    core_error, cr_error, get_last_error, hstring_from_buffer, split_wide_strings, wide_to_string,
    ScopeHandle,
};
use crate::errors::{Error, Result};

#[derive(PartialEq, Eq, Debug)]
pub enum RawDeviceType {
    MOUSE,
    KEYBOARD,
    HID,
    UNKNOWN,
}

impl RawDeviceType {
    pub fn from_rid(t: RID_DEVICE_INFO_TYPE) -> Self {
        match t {
            RIM_TYPEMOUSE => RawDeviceType::MOUSE,
            RIM_TYPEKEYBOARD => RawDeviceType::KEYBOARD,
            RIM_TYPEHID => RawDeviceType::HID,
            _ => RawDeviceType::UNKNOWN,
        }
    }
}

impl fmt::Display for RawDeviceType {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}", format!("{:?}", self).to_lowercase())
    }
}

pub struct RawinputInfo {
    pub rid_info: RID_DEVICE_INFO,
    pub iface: HSTRING,
}

impl RawinputInfo {
    #[inline]
    pub fn typ(&self) -> RawDeviceType {
        RawDeviceType::from_rid(self.rid_info.dwType)
    }
    #[inline]
    pub fn get_mouse(&self) -> &RID_DEVICE_INFO_MOUSE {
        assert!(self.typ() == RawDeviceType::MOUSE);
        unsafe { &self.rid_info.Anonymous.mouse }
    }
    #[inline]
    pub fn get_hid(&self) -> &RID_DEVICE_INFO_HID {
        assert!(self.typ() == RawDeviceType::HID);
        unsafe { &self.rid_info.Anonymous.hid }
    }
}

pub enum WStringOption {
    Some(HSTRING),
    NoValue,
    GetErr(Error),
}

impl Display for WStringOption {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WStringOption::Some(s) => write!(f, "{}", wide_to_string(s)),
            WStringOption::NoValue => write!(f, "NoValue"),
            WStringOption::GetErr(e) => write!(f, "GetPropErr({})", e),
        }
    }
}

pub struct HidDeviceInfo {
    pub serial_number: WStringOption,
    pub manufacturer: WStringOption,
    pub product: WStringOption,
}

pub struct DeviceIfaceInfo {
    pub instance_id: HSTRING,
    pub name: WStringOption,
    pub service: WStringOption,
    pub class: WStringOption,
    pub manufacurer: WStringOption,
}

pub fn device_list_all() -> Result<Vec<RAWINPUTDEVICELIST>> {
    let mut cnt: u32 = 0;
    let mut dev_list: Vec<RAWINPUTDEVICELIST> = Vec::new();

    let res =
        unsafe { GetRawInputDeviceList(None, &mut cnt, size_of::<RAWINPUTDEVICELIST>() as u32) };
    if res == u32::MAX {
        return Err(get_last_error("GetRawInputDeviceList"));
    }

    loop {
        dev_list.resize(cnt as usize, RAWINPUTDEVICELIST::default());

        let res = unsafe {
            GetRawInputDeviceList(
                Some(dev_list.as_mut_ptr()),
                &mut cnt,
                size_of::<RAWINPUTDEVICELIST>() as u32,
            )
        };
        if res != u32::MAX {
            dev_list.shrink_to(res as usize);
            return Ok(dev_list);
        }

        let e = unsafe { GetLastError() };
        if e != ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
    }
}

pub fn device_get_rawinput_rid_info(handle: HANDLE) -> Result<RID_DEVICE_INFO> {
    let mut dst = RID_DEVICE_INFO::default();
    let buffer_size = size_of_val(&dst) as u32;
    let mut size = buffer_size;
    let r = unsafe {
        GetRawInputDeviceInfoW(
            handle,
            RIDI_DEVICEINFO,
            Some((&mut dst as *mut RID_DEVICE_INFO).cast()),
            &mut size,
        )
    };
    if r == u32::MAX {
        if size <= buffer_size {
            return Err(get_last_error("GetRawInputDeviceInfoW"));
        }
        return Err(Error::WinPredefineBufSmall(buffer_size, size));
    }
    Ok(dst)
}

fn device_get_rawinput_info(handle: HANDLE) -> Result<HSTRING> {
    // RIDI_DEVICENAME reports UTF-16 element counts, unlike the byte-based property APIs.
    let mut size: u32 = 0;
    let r = unsafe { GetRawInputDeviceInfoW(handle, RIDI_DEVICENAME, None, &mut size) };
    if r != 0 {
        return Err(get_last_error("GetRawInputDeviceInfoW"));
    }

    let mut buf = vec![0u16; size as usize];
    loop {
        let r = unsafe {
            GetRawInputDeviceInfoW(
                handle,
                RIDI_DEVICENAME,
                Some(buf.as_mut_ptr().cast()),
                &mut size,
            )
        };
        if r == u32::MAX {
            if size as usize <= buf.len() {
                return Err(get_last_error("GetRawInputDeviceInfoW"));
            }
            buf.resize(size as usize, 0);
            continue;
        }
        return hstring_from_buffer(&buf[..size as usize]);
    }
}

pub fn device_collect_rawinput_infos(dev_handle: HANDLE) -> Result<RawinputInfo> {
    Ok(RawinputInfo {
        rid_info: device_get_rawinput_rid_info(dev_handle)?,
        iface: device_get_rawinput_info(dev_handle)?,
    })
}

fn device_get_iface_prop(iface: &HSTRING, key: DEVPROPKEY) -> Result<Option<HSTRING>> {
    let mut size: u32 = 0;
    let mut mtyp = DEVPROP_TYPE_STRING;

    let cr =
        unsafe { CM_Get_Device_Interface_PropertyW(iface, &key, &mut mtyp, None, &mut size, 0) };
    match cr {
        CR_BUFFER_SMALL | CR_SUCCESS => {
            if mtyp != DEVPROP_TYPE_STRING {
                return Err(cr_error(cr));
            }
        }
        CR_NO_SUCH_VALUE => return Ok(None),
        _ => return Err(cr_error(cr)),
    }

    let mut buf = vec![0u16; (size as usize).div_ceil(size_of::<u16>())];
    let cr = unsafe {
        CM_Get_Device_Interface_PropertyW(
            iface,
            &key,
            &mut mtyp,
            Some(buf.as_mut_ptr().cast()),
            &mut size,
            0,
        )
    };
    match cr {
        CR_SUCCESS => {
            if mtyp != DEVPROP_TYPE_STRING {
                return Err(cr_error(cr));
            }
            hstring_from_buffer(&buf[..size as usize / size_of::<u16>()]).map(Some)
        }
        _ => Err(cr_error(cr)),
    }
}

fn device_get_node_prop(devinst: u32, key: DEVPROPKEY) -> Result<Option<HSTRING>> {
    let mut size: u32 = 0;
    let mut mtyp = DEVPROP_TYPE_STRING;

    let cr = unsafe { CM_Get_DevNode_PropertyW(devinst, &key, &mut mtyp, None, &mut size, 0) };
    match cr {
        CR_BUFFER_SMALL | CR_SUCCESS => {
            if mtyp != DEVPROP_TYPE_STRING {
                return Err(cr_error(cr));
            }
        }
        CR_NO_SUCH_VALUE => return Ok(None),
        _ => return Err(cr_error(cr)),
    }

    let mut buf = vec![0u16; (size as usize).div_ceil(size_of::<u16>())];
    let cr = unsafe {
        CM_Get_DevNode_PropertyW(
            devinst,
            &key,
            &mut mtyp,
            Some(buf.as_mut_ptr().cast()),
            &mut size,
            0,
        )
    };
    match cr {
        CR_SUCCESS => {
            if mtyp != DEVPROP_TYPE_STRING {
                return Err(cr_error(cr));
            }
            hstring_from_buffer(&buf[..size as usize / size_of::<u16>()]).map(Some)
        }
        _ => Err(cr_error(cr)),
    }
}

pub fn locate_devnode_handle(instance_id: &HSTRING) -> Result<u32> {
    let mut handle: u32 = 0;
    let cr = unsafe { CM_Locate_DevNodeW(&mut handle, instance_id, CM_LOCATE_DEVNODE_NORMAL) };
    match cr {
        CR_SUCCESS => Ok(handle),
        _ => Err(cr_error(cr)),
    }
}

pub fn device_get_iface_infos(iface: &HSTRING) -> Result<DeviceIfaceInfo> {
    let instance_id = match device_get_iface_prop(iface, DEVPKEY_Device_InstanceId)? {
        Some(v) => v,
        None => return Err(Error::WinDeviceNoInstanceID(wide_to_string(iface))),
    };
    let devinst = locate_devnode_handle(&instance_id)?;

    let getf = |key| -> WStringOption {
        match device_get_node_prop(devinst, key) {
            Ok(opt) => match opt {
                Some(v) => WStringOption::Some(v),
                None => WStringOption::NoValue,
            },
            Err(e) => WStringOption::GetErr(e),
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
    instance_id: &HSTRING,
    class_guid: &GUID,
    present: bool,
) -> Result<Vec<HSTRING>> {
    loop {
        let mut size: u32 = 0;
        let cr = unsafe {
            CM_Get_Device_Interface_List_SizeW(
                &mut size,
                class_guid,
                instance_id,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            )
        };
        match cr {
            CR_SUCCESS => (),
            _ => return Err(cr_error(cr)),
        }

        let mut buf = vec![0u16; size as usize];
        let pre_flag = if present {
            CM_GET_DEVICE_INTERFACE_LIST_PRESENT
        } else {
            CM_GET_DEVICE_INTERFACE_LIST_ALL_DEVICES
        };
        let cr = unsafe {
            CM_Get_Device_Interface_ListW(class_guid, instance_id, buf.as_mut_slice(), pre_flag)
        };
        match cr {
            CR_SUCCESS => return split_wide_strings(&buf),
            CR_BUFFER_SMALL => {
                continue;
            }
            _ => return Err(cr_error(cr)),
        }
    }
}

pub fn device_get_parents(instance_id: &HSTRING, dep_limit: Option<usize>) -> Result<Vec<HSTRING>> {
    let get_parent = |inst_id: &HSTRING| -> Result<Option<HSTRING>> {
        let handle = locate_devnode_handle(inst_id)?;
        device_get_node_prop(handle, DEVPKEY_Device_Parent)
    };

    let mut ret = Vec::<HSTRING>::new();
    let mut inst = instance_id;
    loop {
        if dep_limit.is_some() && ret.len() >= dep_limit.unwrap() {
            break;
        }
        match get_parent(inst)? {
            Some(v) => {
                ret.push(v);
                inst = ret.last().unwrap();
            }
            None => break,
        }
    }
    Ok(ret)
}

pub fn device_open_iface(iface: &HSTRING, metaonly: bool) -> Result<ScopeHandle> {
    let desire_access = if metaonly {
        0
    } else {
        (GENERIC_READ | GENERIC_WRITE).0
    };
    let share_mode = FILE_SHARE_READ | FILE_SHARE_WRITE;

    let result = unsafe {
        CreateFileW(
            iface,
            desire_access,
            share_mode,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )
    };

    match result {
        Ok(h) => {
            if h.is_invalid() {
                Err(Error::WinInvalidHandle(h.0 as isize))
            } else {
                Ok(ScopeHandle::new(h))
            }
        }
        Err(e) => Err(core_error(e, "CreateFileW")),
    }
}

pub fn device_get_hid_info(instance_id: &HSTRING, present: bool) -> Result<HidDeviceInfo> {
    let hid_class = unsafe { HidD_GetHidGuid() };
    let ifaces = device_get_ifaces_list(instance_id, &hid_class, present)?;
    let iface = match ifaces.last() {
        Some(v) => v,
        None => {
            return Err(Error::WinDeviceInterfaceListEmpty(wide_to_string(
                instance_id,
            )))
        }
    };

    let iface_hdl = device_open_iface(iface, true)?;

    let mut data = [0u16; 128];
    let buffer_size = size_of_val(&data) as u32;
    let result_as_str = |ok: BOOLEAN, buf: &[u16]| -> WStringOption {
        if ok.as_bool() {
            match hstring_from_buffer(buf) {
                Ok(value) => WStringOption::Some(value),
                Err(error) => WStringOption::GetErr(error),
            }
        } else {
            WStringOption::NoValue
        }
    };

    let r = HidDeviceInfo {
        serial_number: result_as_str(
            unsafe {
                HidD_GetSerialNumberString(*iface_hdl.get(), data.as_mut_ptr().cast(), buffer_size)
            },
            &data,
        ),
        manufacturer: result_as_str(
            unsafe {
                HidD_GetManufacturerString(*iface_hdl.get(), data.as_mut_ptr().cast(), buffer_size)
            },
            &data,
        ),
        product: result_as_str(
            unsafe {
                HidD_GetProductString(*iface_hdl.get(), data.as_mut_ptr().cast(), buffer_size)
            },
            &data,
        ),
    };

    Ok(r)
}
