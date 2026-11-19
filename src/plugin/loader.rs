use std::{
    fs, io,
    path::{Path, PathBuf},
};

use libloading::os::windows::{
    Library, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
};
use monmouse_lib::plugin::{PluginDefine, RegisterPlugin};

fn plugin_paths() -> io::Result<Vec<PathBuf>> {
    let executable = std::env::current_exe()?;
    let entries = match fs::read_dir(executable.with_file_name("plugins")) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("dll"))
            && entry.file_type()?.is_file()
        {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

pub fn load() -> io::Result<Vec<PluginDefine>> {
    let mut plugins: Vec<PluginDefine> = Vec::new();
    for path in plugin_paths()? {
        match load_plugin(&path) {
            Ok((plugin, dll)) => {
                if plugins.iter().any(|loaded| loaded.key == plugin.key) {
                    log::warn!(
                        "Skipping {}: duplicate plugin key {}",
                        path.display(),
                        plugin.key
                    );
                    drop(plugin);
                    continue;
                }
                // Registered tabs, tasks and messages can use DLL code until process exit.
                std::mem::forget(dll);
                plugins.push(plugin);
            }
            Err(error) => log::warn!("Could not load plugin {}: {error}", path.display()),
        }
    }
    Ok(plugins)
}

fn load_plugin(path: &Path) -> Result<(PluginDefine, Library), libloading::Error> {
    let dll = unsafe {
        Library::load_with_flags(
            path,
            LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
        )?
    };
    let register = unsafe { *dll.get::<RegisterPlugin>(b"register_plugin_v1\0")? };
    Ok((register(), dll))
}
