//! Build script for vmaf-head-sys.
//!
//! Compiles the vendored libvmaf library using Meson.

use std::{env, ffi::OsStr, io::ErrorKind, path::PathBuf, process::Command};

fn main() {
    if let Err(error) = build_vmaf() {
        panic!("Failed to build VMAF: {error}");
    }
}

fn build_vmaf() -> Result<(), Box<dyn std::error::Error>> {
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let is_x86 = matches!(target_arch.as_str(), "x86" | "x86_64");
    if env::var_os("DOCS_RS").is_some() || target_arch.starts_with("wasm") {
        return Ok(());
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let source_dir = manifest_dir.join("vendored/vmaf/libvmaf");
    let build_dir = out_dir.join("build");
    let install_dir = out_dir.join("install");

    if !source_dir.is_dir() {
        return Err(format!(
            "Missing VMAF source directory: {}. Run 'python vendor_vmaf.py' to download it.",
            source_dir.display()
        )
        .into());
    }

    println!("cargo:rerun-if-changed=vendored/vmaf");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");
    println!("cargo:rerun-if-env-changed=MESON");
    println!("cargo:rerun-if-env-changed=NINJA");
    println!("cargo:rerun-if-env-changed=VMAF_MESON_CROSS_FILE");

    let meson = env::var_os("MESON").unwrap_or_else(|| "meson".into());
    let ninja = env::var_os("NINJA").unwrap_or_else(|| "ninja".into());
    require_build_tool(&meson, "Meson", "MESON")?;
    require_build_tool(&ninja, "Ninja", "NINJA")?;
    if is_x86 {
        require_nasm()?;
    }

    let asm_enabled = is_x86 || env::var_os("CARGO_FEATURE_ASM").is_some();

    let mut setup = Command::new(&meson);
    setup
        .arg("setup")
        .arg(&build_dir)
        .arg(&source_dir)
        .arg(format!("--prefix={}", install_dir.display()))
        .arg("--libdir=lib")
        .arg("--default-library=static")
        .arg("--buildtype=release")
        .arg("-Denable_tests=false")
        .arg("-Denable_docs=false")
        .arg("-Denable_tools=false")
        .arg("-Denable_cuda=false")
        .arg("-Denable_nvtx=false")
        .arg(feature_option("built-in-models", "built_in_models"))
        .arg(format!("-Denable_asm={asm_enabled}"))
        .arg(feature_option("float", "enable_float"));

    if let Some(cross_file) = env::var_os("VMAF_MESON_CROSS_FILE") {
        setup.arg("--cross-file").arg(cross_file);
    }

    let setup_status = setup.status()?;

    if !setup_status.success() {
        return Err("Meson setup failed".into());
    }

    let compile_status = Command::new(&meson)
        .args(["compile", "-C"])
        .arg(&build_dir)
        .status()?;
    if !compile_status.success() {
        return Err("Meson compile failed".into());
    }

    let install_status = Command::new(&meson)
        .args(["install", "-C"])
        .arg(&build_dir)
        .arg("--no-rebuild")
        .status()?;
    if !install_status.success() {
        return Err("Meson install failed".into());
    }

    println!(
        "cargo:rustc-link-search=native={}",
        install_dir.join("lib").display()
    );
    println!("cargo:rustc-link-lib=static=vmaf");

    let target_os = env::var("CARGO_CFG_TARGET_OS")?;
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    match (target_os.as_str(), target_env.as_str()) {
        ("macos" | "ios", _) => println!("cargo:rustc-link-lib=c++"),
        ("windows", "msvc") => println!("cargo:rustc-link-lib=msvcp140"),
        ("windows", _) => println!("cargo:rustc-link-lib=stdc++"),
        _ => println!("cargo:rustc-link-lib=stdc++"),
    }

    if matches!(target_os.as_str(), "linux" | "android" | "freebsd") {
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=pthread");
    }

    Ok(())
}

fn feature_option(feature: &str, option: &str) -> String {
    let cargo_feature = format!("CARGO_FEATURE_{}", feature.replace('-', "_").to_uppercase());
    let value = if env::var_os(cargo_feature).is_some() {
        "true"
    } else {
        "false"
    };
    format!("-D{option}={value}")
}

fn require_build_tool(
    executable: &OsStr,
    name: &str,
    override_variable: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    match Command::new(executable).arg("--version").output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(format!(
            "{name} executable '{}' failed its version check with status {}. Install a working {name} executable or set {override_variable} to its path.",
            executable.to_string_lossy(),
            output.status
        )
        .into()),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(format!(
            "{name} is required to build vmaf-head-sys, but executable '{}' was not found. Install {name} and ensure it is on PATH, or set {override_variable} to its path.",
            executable.to_string_lossy()
        )
        .into()),
        Err(error) => Err(format!(
            "Failed to run {name} executable '{}': {error}. Install a working {name} executable or set {override_variable} to its path.",
            executable.to_string_lossy()
        )
        .into()),
    }
}

fn require_nasm() -> Result<(), Box<dyn std::error::Error>> {
    match Command::new("nasm").arg("-v").output() {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err(format!(
            "NASM failed its version check with status {}. A working NASM installation is required for x86 and x86_64 builds.",
            output.status
        )
        .into()),
        Err(error) if error.kind() == ErrorKind::NotFound => Err(
            "NASM is required for x86 and x86_64 builds, but `nasm` was not found. Install NASM and ensure it is on PATH."
                .into(),
        ),
        Err(error) => Err(format!(
            "Failed to run NASM: {error}. Install NASM and ensure `nasm` is on PATH."
        )
        .into()),
    }
}
