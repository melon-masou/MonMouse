use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("ErrorNoConfigFile")]
    ConfigFileNotExists,
    #[error("ErrorIO({0})")]
    IO(std::io::Error),
    #[error("ErrorInvalidConfigFile({0})")]
    InvalidConfigFile(String),
    #[error("ErrorInvalidParam(field={0}; {1})")]
    InvalidParam(String, String),
    #[error("ErrorInvalidShortCut({0})")]
    InvalidShortcut(String),

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

pub type Result<T> = std::result::Result<T, Error>;
