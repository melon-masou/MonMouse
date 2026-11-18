use log::error;
use windows::{
    core::HSTRING,
    Win32::{Foundation::HANDLE, UI::Input::RAWINPUTDEVICELIST},
};

use super::winwrap::{devices::*, wide_to_string};
use crate::{device_type::DeviceType, errors::Result, message::GenericDevice};

pub struct WinDevice {
    pub handle: HANDLE,
    pub device_type: DeviceType,
    pub id: Option<String>,
    pub rawinput: Option<RawinputInfo>,
    pub iface: Option<DeviceIfaceInfo>,
    pub parents: Vec<HSTRING>,
    pub hid: Option<HidDeviceInfo>,
}

impl std::fmt::Debug for WinDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::fmt::Display for WinDevice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Dev({:?})", self.handle.0)?;

        let rawinput = match &self.rawinput {
            Some(v) => v,
            None => return writeln!(f, "Is a dummy device"),
        };

        writeln!(f, "iface: {}", wide_to_string(&rawinput.iface))?;
        writeln!(f, "dwType: {}", rawinput.typ())?;
        write!(f, "parents: [ ")?;
        for p in &self.parents {
            write!(f, "{} ", wide_to_string(p))?;
        }
        writeln!(f, "]")?;

        match rawinput.typ() {
            RawDeviceType::MOUSE => {
                let mouse = rawinput.get_mouse();
                writeln!(f, "Is a Mouse")?;
                writeln!(f, "dwId: {}", mouse.dwId)?;
                writeln!(f, "dwNumberOfButtons: {}", mouse.dwNumberOfButtons)?;
                writeln!(f, "dwSampleRate: {}", mouse.dwSampleRate)?;
            }
            RawDeviceType::KEYBOARD => {
                let hid = rawinput.get_hid();
                writeln!(f, "Is HID")?;
                writeln!(f, "dwProductId: {}", hid.dwProductId)?;
                writeln!(f, "dwVendorId: {}", hid.dwVendorId)?;
                writeln!(f, "dwVersionNumber: {}", hid.dwVersionNumber)?;
                writeln!(f, "usUsagePage: {}", hid.usUsagePage)?;
                writeln!(f, "usUsage: {}", hid.usUsage)?;
            }
            _ => (),
        }
        if let Some(infos) = &self.iface {
            writeln!(f, "iface info::")?;
            writeln!(f, "instance_id: {}", wide_to_string(&infos.instance_id))?;
            writeln!(f, "name: {}", infos.name)?;
            writeln!(f, "service: {}", infos.service)?;
            writeln!(f, "class: {}", infos.class)?;
            writeln!(f, "manufacurer: {}", infos.manufacurer)?;
        };
        if let Some(infos) = &self.hid {
            writeln!(f, "hid info::")?;
            writeln!(f, "serial_number: {}", infos.serial_number)?;
            writeln!(f, "product: {}", infos.product)?;
            writeln!(f, "manufacturer: {}", infos.manufacturer)?;
        };
        Ok(())
    }
}

fn get_device_type(rawinput: &RawinputInfo) -> DeviceType {
    match rawinput.typ() {
        RawDeviceType::MOUSE => DeviceType::Mouse,
        RawDeviceType::KEYBOARD => DeviceType::Keyboard,
        RawDeviceType::HID => {
            let hid = rawinput.get_hid();
            DeviceType::from_hid_usage(hid.usUsagePage, hid.usUsage)
        }
        RawDeviceType::UNKNOWN => DeviceType::Unknown,
    }
}

fn collect_rawinput_infos(dev: &RAWINPUTDEVICELIST) -> Result<RawinputInfo> {
    let handlev = dev.hDevice.0;
    match device_collect_rawinput_infos(dev.hDevice) {
        Ok(v) => Ok(v),
        Err(e) => {
            error!("Get dev info failed({:?}): {}", handlev, e);
            Err(e)
        }
    }
}

fn collect_device_infos(
    handle: HANDLE,
    device_type: DeviceType,
    rawinput: RawinputInfo,
) -> Result<WinDevice> {
    let handlev = handle.0;
    let (iface, id) = match device_get_iface_infos(&rawinput.iface) {
        Ok(v) => {
            let id = wide_to_string(&v.instance_id);
            (Some(v), Some(id))
        }
        Err(e) => {
            error!(
                "Get iface info failed({:?}): {}. interface={}",
                handlev,
                e,
                wide_to_string(&rawinput.iface),
            );
            (None, None)
        }
    };
    let parents = match &iface {
        Some(i) => match device_get_parents(&i.instance_id, None) {
            Ok(v) => v,
            Err(e) => {
                error!(
                    "Get device parents failed({:?}): {}. interface={}",
                    handlev,
                    e,
                    wide_to_string(&rawinput.iface),
                );
                Vec::new()
            }
        },
        None => Vec::new(),
    };
    let hid = match (&iface, rawinput.typ()) {
        (Some(i), RawDeviceType::HID) => match device_get_hid_info(&i.instance_id, true) {
            Ok(v) => Some(v),
            Err(e) => {
                error!(
                    "Get hid info failed({:?}): {:?}. interface={}",
                    handlev,
                    e,
                    wide_to_string(&rawinput.iface)
                );
                None
            }
        },
        _ => None,
    };
    Ok(WinDevice {
        handle,
        id,
        device_type,
        rawinput: Some(rawinput),
        iface,
        parents,
        hid,
    })
}

fn filter_rawinput_devices(device_type: DeviceType) -> bool {
    device_type.is_pointer()
}

pub fn collect_all_raw_devices() -> Result<Vec<WinDevice>> {
    let all_devs = device_list_all()?;
    Ok(all_devs
        .into_iter()
        .filter_map(|d| {
            let rawinput = match collect_rawinput_infos(&d) {
                Ok(v) => v,
                Err(e) => {
                    error!("Failed to collect rawinput info({:?}): {}", d.hDevice.0, e);
                    return None;
                }
            };
            let device_type = get_device_type(&rawinput);
            if !filter_rawinput_devices(device_type) {
                return None;
            }
            match collect_device_infos(d.hDevice, device_type, rawinput) {
                Ok(v) => Some(v),
                Err(e) => {
                    error!("Failed to collect device info({:?}): {}", d.hDevice.0, e);
                    None
                }
            }
        })
        .collect())
}

pub fn is_valid_win_device(d: &WinDevice) -> bool {
    d.id.is_some()
}

pub fn win_device_to_generic(d: &WinDevice) -> GenericDevice {
    GenericDevice {
        id: d.id.as_ref().unwrap().to_string(),
        device_type: d.device_type,
        product_name: build_product_name(d).trim().into(),
        platform_specific_infos: build_platform_specific_infos(d),
    }
}

fn build_product_name(d: &WinDevice) -> String {
    if let Some(hid) = &d.hid {
        let mut name = String::new();
        if let WStringOption::Some(s) = &hid.manufacturer {
            name.push_str(wide_to_string(s).as_str());
            name.push(' ');
        }
        if let WStringOption::Some(s) = &hid.product {
            name.push_str(wide_to_string(s).as_str());
            name.push(' ');
        }
        if !name.is_empty() {
            return name;
        }
    };
    if let Some(iface) = &d.iface {
        let mut name = if let WStringOption::Some(s) = &iface.manufacurer {
            let mut s = wide_to_string(s);
            s.push(' ');
            s
        } else {
            String::new()
        };
        name.push_str(iface.name.to_string().as_str());
        return name;
    }
    d.id.as_ref().unwrap().clone()
}

fn build_platform_specific_infos(d: &WinDevice) -> Vec<(String, String)> {
    let tag = |s: &str| s.to_owned();

    let rawinput = match &d.rawinput {
        Some(v) => v,
        None => return Vec::new(),
    };

    let mut vs = vec![
        (tag("interface"), wide_to_string(&rawinput.iface)),
        (tag("dwType"), rawinput.rid_info.dwType.0.to_string()),
    ];
    if let Some(hm) = &d.hid {
        if let WStringOption::Some(s) = &hm.manufacturer {
            vs.push((tag("hidManufacurer"), wide_to_string(s)));
        }
        if let WStringOption::Some(s) = &hm.product {
            vs.push((tag("hidProduct"), wide_to_string(s)));
        }
        if let WStringOption::Some(s) = &hm.serial_number {
            vs.push((tag("hidSerialNumber"), wide_to_string(s)));
        }
    }

    if let Some(im) = &d.iface {
        if let WStringOption::Some(s) = &im.manufacurer {
            vs.push((tag("manufacurer"), wide_to_string(s)));
        }
        if let WStringOption::Some(s) = &im.name {
            vs.push((tag("name"), wide_to_string(s)));
        }
        if let WStringOption::Some(s) = &im.service {
            vs.push((tag("service"), wide_to_string(s)));
        }
        if let WStringOption::Some(s) = &im.class {
            vs.push((tag("class"), wide_to_string(s)));
        }
    }

    match rawinput.typ() {
        RawDeviceType::MOUSE => {
            let m = &rawinput.get_mouse();
            vs.push((tag("dwId"), m.dwId.to_string()));
            vs.push((tag("dwNumberOfButtons"), m.dwNumberOfButtons.to_string()));
            vs.push((tag("dwSampleRate"), m.dwSampleRate.to_string()));
        }
        RawDeviceType::KEYBOARD => (),
        RawDeviceType::HID => {
            let m = &rawinput.get_hid();
            vs.push((tag("dwProductId"), m.dwProductId.to_string()));
            vs.push((tag("dwVendorId"), m.dwVendorId.to_string()));
            vs.push((tag("dwVersionNumber"), m.dwVersionNumber.to_string()));
            vs.push((tag("usUsagePage"), format!("0x{:X}", m.usUsagePage)));
            vs.push((tag("usUsage"), format!("0x{:X}", m.usUsage)));
        }
        RawDeviceType::UNKNOWN => (),
    }

    vs
}
