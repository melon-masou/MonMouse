use monmouse::errors::Error;
use std::path::PathBuf;

#[cfg(target_os = "windows")]
#[cfg(not(debug_assertions))]
pub fn get_config_dir() -> Result<PathBuf, Error> {
    match std::env::current_dir().map(PathBuf::from) {
        Ok(v) => Ok(v),
        Err(_) => Err(Error::ConfigFileNotExists("None".to_owned())),
    }
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
