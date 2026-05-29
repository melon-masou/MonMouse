use monmouse::errors::Error;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
#[cfg(not(debug_assertions))]
pub fn get_config_dir() -> Result<PathBuf, Error> {
    std::env::current_exe()
        .ok()
        .and_then(|v| v.parent().map(PathBuf::from))
        .ok_or_else(|| Error::ConfigFileNotExists("current exe dir".to_owned()))
}

#[cfg(debug_assertions)]
pub fn get_config_dir() -> Result<PathBuf, Error> {
    Ok(PathBuf::from("debug"))
}

// #[cfg(target_os = "windows")]
// pub fn get_config_dir() -> Option<PathBuf> {
//     std::env::var_os("APPDATA")
//         .map(PathBuf::from)
//         .map(|v| v.join("monmouse"))
// }
