use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/framelatch.ico");
    println!("cargo:rerun-if-changed=assets/framelatch.rc");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let sdk_bin = PathBuf::from(
        env::var_os("ProgramFiles(x86)").expect("Windows SDK location is unavailable"),
    )
    .join("Windows Kits")
    .join("10")
    .join("bin");
    let mut versions: Vec<_> = fs::read_dir(&sdk_bin)
        .expect("Windows SDK is required to compile the application icon")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect();
    versions.sort();
    let arch = if env::consts::ARCH == "aarch64" {
        "arm64"
    } else {
        "x64"
    };
    let rc = versions
        .into_iter()
        .rev()
        .map(|path| path.join(arch).join("rc.exe"))
        .find(|path| path.is_file())
        .expect("Windows SDK resource compiler rc.exe was not found");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is unavailable"))
        .join("framelatch.res");
    let status = Command::new(rc)
        .arg("/nologo")
        .arg(format!("/fo{}", output.display()))
        .arg("assets/framelatch.rc")
        .status()
        .expect("Could not start Windows resource compiler");
    assert!(
        status.success(),
        "Windows resource compilation failed: {status}"
    );
    println!("cargo:rustc-link-arg={}", output.display());
}
