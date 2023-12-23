use std::fmt::Display;

use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("ErrorNoConfigFile(path={0})")]
    ConfigFileNotExists(String),
    #[error("ErrorIO({0})")]
    IO(std::io::Error),
    #[error("ErrorInvalidConfigFile({0})")]
    InvalidConfigFile(String),
    #[error("ErrorInvalidParam(field={0}; {1})")]
    InvalidParam(String, String),
    #[error("ErrorInvalidShortCut({0})")]
    InvalidShortcut(String),
    #[error("ErrorShortCutConflict({0})")]
    ShortcutConflict(PrintableOptionString),

    #[error("ErrorInited")]
    MessageInited,

    #[error("ErrorWinUnknown")]
    WinUnknown,
    #[error("ErrorWinCore(code=0x{0:X})")]
    WinCore(i32),
    #[error("ErrorWinConfigRet(cr={0})")]
    WinConfigRet(u32),
    #[error("ErrorWinPredefineBufSmall(get={0},need={1})")]
    WinPredefineBufSmall(u32, u32),
    #[error("ErrorWinDeviceNoInstanceID(interface={0})")]
    WinDeviceNoInstanceID(String),
    #[error("ErrorWinDeviceNoInterface(instance_id={0})")]
    WinDeviceInterfaceListEmpty(String),
    #[error("ErrorWinInvalidHandle(v={0})")]
    WinInvalidHandle(isize),
}

#[derive(Debug)]
pub struct PrintableOptionString(Option<String>);

impl Display for PrintableOptionString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let PrintableOptionString(Some(v)) = self {
            write!(f, "{}", v)
        } else {
            Ok(())
        }
    }
}

impl From<&str> for PrintableOptionString {
    fn from(value: &str) -> Self {
        PrintableOptionString(Some(value.to_owned()))
    }
}

impl From<String> for PrintableOptionString {
    fn from(value: String) -> Self {
        PrintableOptionString(Some(value))
    }
}

impl From<Option<String>> for PrintableOptionString {
    fn from(value: Option<String>) -> Self {
        PrintableOptionString(value)
    }
}

pub type Result<T> = std::result::Result<T, Error>;
