use std::{env, fs::File, io::Write, path::PathBuf, process};
use which::which;

// macro_rules! warn {
//     ($($tokens: tt)*) => {
//         println!("cargo:warning={}", format!($($tokens)*))
//     }
// }

fn run_command(tag: &str, command: &mut process::Command) -> process::Output {
    let output = command.output().unwrap();
    println!(
        "----- {tag} stdout:\n{}\n",
        String::from_utf8_lossy(&output.stdout)
    );
    println!(
        "----- {tag} stderr:\n{}\n",
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn generate_rc(add_icon: bool) -> String {
    #[cfg(not(any(debug_assertions, feature = "dep-only")))]
    let rc_template = include_bytes!("assets\\monmouse.rc");
    #[cfg(any(debug_assertions, feature = "dep-only"))]
    let rc_template = [];

    let version = env::var("CARGO_PKG_VERSION").unwrap();
    let version_num = version.replace('.', ",");

    let mut rc = String::from_utf8(rc_template.to_vec()).unwrap();
    rc = rc
        .replacen("FILL_VERSION_NUM", &version_num, 1)
        .replacen("FILL_VERSION_STR", &version, 1);
    if add_icon {
        rc = rc.replacen("FILL_ADD_ICON", "ADD_ICON", 1);
    }
    rc
}

fn windows_rc_compile(content: String, out_dir: &str, rc_file: &str, lib_file: &str) {
    let rc_file = PathBuf::from(out_dir).join(rc_file);
    let lib_file = PathBuf::from(out_dir).join(lib_file);

    let mut write_file = File::create(&rc_file).unwrap();
    write!(write_file, "{}", content).unwrap();

    let rc_exe = env::var("RC_EXEC")
        .map(PathBuf::from)
        .or_else(|_| which("rc"))
        .unwrap();

    let output = run_command(
        "rc",
        process::Command::new(rc_exe)
            .arg(format!("/fo{}", lib_file.display()))
            .arg(format!("{}", rc_file.display())),
    );
    if !output.status.success() {
        panic!("rc compile error");
    }
}

fn main() {
    let anno = match env::var("VERSION_ANNO").unwrap_or_default().as_str() {
        "release" => "",
        "" => "dev",
        v => v,
    }
    .to_string();
    println!("cargo:rustc-env=VERSION_ANNO={}", anno);
    let sha = env::var("VERSION_SHA").unwrap_or_default();
    println!(
        "cargo:rustc-env=VERSION_SHA={}",
        if sha.len() <= 7 { &sha[..] } else { &sha[..7] }
    );

    if cfg!(feature = "dep-only") {
        println!("cargo::rerun-if-changed=dep-only")
    }

    if cfg!(target_os = "windows") && cfg!(not(any(debug_assertions, feature = "dep-only"))) {
        // let mut res = winres::WindowsResource::new();
        // res.compile().unwrap();
        let _manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
        let out_dir = env::var("OUT_DIR").unwrap();

        windows_rc_compile(generate_rc(true), &out_dir, "res.rc", "res.lib");
        windows_rc_compile(generate_rc(false), &out_dir, "res-cli.rc", "res-cli.lib");

        println!("cargo:rustc-link-search=native={}", out_dir);
        // link to all binarys
        // println!("cargo:rustc-link-lib=dylib=res");
        // link to single binary
        println!("cargo:rustc-link-arg-bin=monmouse=res.lib");
        println!("cargo:rustc-link-arg-bin=monmouse-cli=res-cli.lib");
    }
}
