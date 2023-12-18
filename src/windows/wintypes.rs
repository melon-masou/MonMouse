use std::mem::size_of;
use std::os::windows::ffi::OsStringExt;
use std::{ffi::OsString, mem};

use windows::Win32::Devices::DeviceAndDriverInstallation::CONFIGRET;
use windows::Win32::Foundation::WPARAM;
use windows::Win32::UI::Input::HRAWINPUT;
use windows::{core::PCWSTR, Win32::Foundation::LPARAM};

use crate::errors::Error;

use super::constants::STR_INVALID_WIN_WIDE_OS_STR;

pub fn wmut_vec<T>(v: &mut Vec<T>) -> *mut T {
    v.as_mut_ptr()
}

pub fn wmut_obj<T>(v: &mut T) -> *mut std::ffi::c_void {
    v as *mut _ as *mut std::ffi::c_void
}

pub fn wmut_buf<T>(v: &mut Vec<T>) -> *mut std::ffi::c_void {
    v.as_mut_ptr() as *mut std::ffi::c_void
}

pub fn wptr<T>(v: &T) -> *const T {
    v as *const T
}

pub fn wmut_ptr<T>(v: &mut T) -> *mut T {
    v as *mut T
}

#[allow(clippy::mut_from_ref)]
#[inline]
pub fn wparam_ref<T>(v: &WPARAM) -> &mut T {
    unsafe { &mut *(v.0 as *mut T) }
}
#[allow(clippy::mut_from_ref)]
#[inline]
pub fn lparam_ref<T>(v: &LPARAM) -> &mut T {
    unsafe { &mut *(v.0 as *mut T) }
}
#[inline]
pub fn lparam_from<T>(v: &mut T) -> LPARAM {
    LPARAM(v as *mut T as isize)
}

pub fn lparam_as_rawinput(lparam: LPARAM) -> HRAWINPUT {
    HRAWINPUT(lparam.0)
}

pub type WSize = u32;
pub fn wsize_of_val<T>(v: &T) -> WSize {
    mem::size_of_val(v) as WSize
}
pub fn wsize_of<T>() -> WSize {
    mem::size_of::<T>() as WSize
}

pub struct WObj {
    pub ptr: *mut std::ffi::c_void,
    pub size: WSize,
}

impl WObj {
    pub fn from<T>(t: &mut T) -> WObj {
        WObj {
            ptr: wmut_obj(t),
            size: wsize_of_val(t),
        }
    }
}

#[inline(always)]
pub fn cr_error(cr: CONFIGRET) -> Error {
    Error::WinConfigRet(cr.0)
}

#[inline(always)]
pub fn core_error(e: ::windows::core::Error) -> Error {
    Error::WinCore(e.code().0)
}

pub trait IBuffer {
    fn new(size: WSize) -> Self;
    fn resize(&mut self, size: WSize);
    fn as_mut_data(&mut self) -> *mut std::ffi::c_void;
    fn capacity(&self) -> WSize;
}

pub struct WBuffer(pub Vec<u8>);

impl IBuffer for WBuffer {
    fn new(size: WSize) -> Self {
        WBuffer(vec![0; size as usize])
    }
    fn resize(&mut self, size: WSize) {
        self.0.resize(size as usize, 0)
    }
    fn as_mut_data(&mut self) -> *mut std::ffi::c_void {
        self.0.as_mut_ptr() as *mut std::ffi::c_void
    }
    fn capacity(&self) -> WSize {
        (self.0.capacity()) as WSize
    }
}

impl WBuffer {
    pub fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr() as *const u16)
    }
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr()
    }
    pub fn get_ref<T>(&self) -> &T {
        std::assert!(self.capacity() as usize >= size_of::<T>());
        unsafe { &*(self.0.as_ptr() as *const T) }
    }

    pub fn to_wstring(self) -> WString {
        WString(
            self.0
                .chunks(2)
                .map(|x| -> u16 {
                    if x.len() < 2 {
                        x[0] as u16
                    } else {
                        ((x[1] as u16) << 8) | (x[0] as u16)
                    }
                })
                .collect(),
        )
    }
}

// Windows wide string representation. Ends with '\0'
pub struct WString(pub Vec<u16>);

impl IBuffer for WString {
    fn new(size: WSize) -> Self {
        WString(vec![0; size as usize])
    }
    fn resize(&mut self, size: WSize) {
        self.0.resize(size as usize, 0)
    }
    fn as_mut_data(&mut self) -> *mut std::ffi::c_void {
        self.0.as_mut_ptr() as *mut std::ffi::c_void
    }
    fn capacity(&self) -> WSize {
        (self.0.capacity()) as WSize
    }
}

impl Clone for WString {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl WString {
    pub fn as_pcwstr(&self) -> PCWSTR {
        PCWSTR(self.0.as_ptr())
    }
    pub fn as_mut_slice(&mut self) -> &mut [u16] {
        self.0.as_mut_slice()
    }
    pub fn as_slice(&self) -> &[u16] {
        self.0.as_slice()
    }
    pub fn as_u8_slice(&self) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.0.as_ptr() as *const u8, self.0.len() * 2) }
    }

    pub fn split_by_eos(&self) -> Vec<WString> {
        let mut r = Vec::<WString>::new();
        let acc = self.0.iter().fold(Vec::<u16>::new(), |mut acc, x| {
            acc.push(*x);
            if *x == 0 {
                if acc.len() > 1 {
                    r.push(WString(acc));
                }
                Vec::new()
            } else {
                acc
            }
        });
        if acc.len() > 1 {
            r.push(WString(acc));
        }
        r
    }
    pub fn str_before_null(&self) -> WString {
        WString(self.0.split(|v| *v == 0).next().unwrap().to_vec())
    }

    pub fn to_wbuffer(&self) -> WBuffer {
        let mut a = Vec::<u8>::new();
        self.0.iter().for_each(|x| {
            a.push((x & std::u8::MAX as u16) as u8);
            a.push((x >> 8) as u8);
        });
        WBuffer(a)
    }
    pub fn encode_from_str(s: &str) -> WString {
        let mut a: Vec<u16> = s.encode_utf16().collect();
        a.push(0);
        WString(a)
    }
}

impl std::fmt::Display for WString {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let before_null = self.as_slice().split(|v| *v == 0).next().unwrap();
        match OsString::from_wide(before_null).into_string() {
            Ok(v) => write!(f, "{}", v),
            Err(_) => write!(f, "{}", STR_INVALID_WIN_WIDE_OS_STR),
        }
    }
}
