use std::{
    ffi::CStr,
    fs, io,
    path::{Path, PathBuf},
};

use libloading::os::windows::{
    Library, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use monmouse_lib::plugin::{
    PluginDefine, ReadlibVersion, RegisterPlugin, LIB_VERSION_SYMBOL, REGISTER_PLUGIN_SYMBOL,
};

pub fn discover() -> io::Result<Vec<(String, PathBuf)>> {
    let mut directory = std::env::current_exe()?;
    directory.pop();

    let mut plugins = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let Some(key) = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem| stem.strip_prefix("monmouse_plugins_"))
        else {
            continue;
        };
        if !key.is_empty()
            && key
                .bytes()
                .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'_'))
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            && entry.file_type()?.is_file()
        {
            plugins.push((key.to_owned(), path));
        }
    }
    plugins.sort();
    plugins.dedup_by(|a, b| a.0 == b.0);
    Ok(plugins)
}

pub fn load(path: &Path, key: &str) -> Result<PluginDefine, String> {
    let (dll, plugin) = load_plugin(path)
        .map_err(|error| format!("Could not load plugin {}: {error}", path.display()))?;
    if plugin.key != key {
        return Err(format!(
            "Plugin {} declares key {:?}, expected {key:?}",
            path.display(),
            plugin.key
        ));
    }

    // Registered tabs, tasks and messages can use DLL code until process exit.
    std::mem::forget(dll);
    Ok(plugin)
}

fn load_plugin(path: &Path) -> Result<(Library, PluginDefine), libloading::Error> {
    let dll = unsafe {
        Library::load_with_flags(
            path,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )?
    };
    let register = unsafe { *dll.get::<RegisterPlugin>(REGISTER_PLUGIN_SYMBOL)? };
    if let Ok(version) = unsafe { dll.get::<ReadlibVersion>(LIB_VERSION_SYMBOL) } {
        // The SDK export returns a NUL-terminated string in the loaded DLL.
        let version = unsafe { CStr::from_ptr(version()) };
        log::info!(
            "{} uses monmouse-lib {}",
            path.display(),
            version.to_string_lossy()
        );
    }
    Ok((dll, register()))
}
