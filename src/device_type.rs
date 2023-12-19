// Ref:
//   https://www.usb.org/document-library/hid-usage-tables-14
//   https://learn.microsoft.com/en-us/windows-hardware/drivers/hid/hid-architecture

use std::fmt::Display;

#[derive(Debug, Clone, Copy)]
pub enum DeviceType {
    Unknown,
    Dummy,
    UnknownHID,

    // Generic Desktop Page(0x01)
    Pointer,
    Mouse,
    Joystick,
    Gamepad,
    Keyboard,
    Keypad,
    OtherGenericDesktop,

    // Digitizer Page(0x0D)
    Digitizer,
    Pen,
    LightPen,
    TouchScreen,
    TouchPad,
    Whiteboard,
    OtherDigitizer,

    // Page: 0xFF00-
    VendorDefined,
}

impl DeviceType {
    pub fn from_hid_usage(page: u16, usage: u16) -> Self {
        if page >= 0xFF00 {
            return Self::VendorDefined;
        }
        match page {
            0x01 => match usage {
                0x01 => Self::Pointer,
                0x02 => Self::Mouse,
                0x04 => Self::Joystick,
                0x05 => Self::Gamepad,
                0x06 => Self::Keyboard,
                0x07 => Self::Keypad,
                _ => Self::OtherGenericDesktop,
            },
            0x0D => match usage {
                0x01 => Self::Digitizer,
                0x02 => Self::Pen,
                0x03 => Self::LightPen,
                0x04 => Self::TouchScreen,
                0x05 => Self::TouchPad,
                0x06 => Self::Whiteboard,
                _ => Self::OtherDigitizer,
            },
            _ => Self::UnknownHID,
        }
    }

    pub fn is_pointer(&self) -> bool {
        matches!(
            self,
            DeviceType::Dummy
                | DeviceType::Pointer
                | DeviceType::Mouse
                | DeviceType::Digitizer
                | DeviceType::Pen
                | DeviceType::LightPen
                | DeviceType::TouchScreen
                | DeviceType::TouchPad
                | DeviceType::Whiteboard
                | DeviceType::OtherDigitizer
        )
    }
}

impl Display for DeviceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub struct WindowsRawinput {}

impl WindowsRawinput {
    pub const ALL: u16 = 0;
    pub const REGISTER_USAGE_SET: [(u16, u16); 3] = [
        (0x0D, Self::ALL), // Digitizer, All
        (0x01, 0x01),      // Generic Desktop, Pointer
        (0x01, 0x02),      // Generic Desktop, Mouse
    ];
}
