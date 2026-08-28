//! Build script for vmaf-head-sys.
//!
//! Compiles the vendored libvmaf library using Meson.

use std::{
    env,
    ffi::OsStr,
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
};

fn main() {
    if let Err(error) = build_vmaf() {
        panic!("Failed to build VMAF: {error}");
    }
}

fn build_vmaf() -> Result<(), Box<dyn std::error::Error>> {
    let host = env::var("HOST")?;
    let target = env::var("TARGET")?;
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let target_features = env::var("CARGO_CFG_TARGET_FEATURE").unwrap_or_default();
    let target_os = env::var("CARGO_CFG_TARGET_OS")?;
    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let is_x86 = matches!(target_arch.as_str(), "x86" | "x86_64");
    let is_msvc = target_os == "windows" && target_env == "msvc";
    let host_build = host == target;
    let encoded_rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    let native_requested = target_cpu_from_rustflags(&encoded_rustflags) == Some("native");
    let native_tuning = native_requested && host_build;
    let compiler_tuning_args = if host_build {
        compiler_tuning_args(&target_arch, is_msvc, native_tuning, &target_features)
    } else {
        Vec::new()
    };
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
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_FEATURE");
    println!("cargo:rerun-if-env-changed=VMAF_MESON_CROSS_FILE");
    println!("cargo:rerun-if-env-changed=IPHONEOS_DEPLOYMENT_TARGET");
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_HOME");
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_ROOT");
    println!("cargo:rerun-if-env-changed=ANDROID_NDK_PATH");
    println!("cargo:rerun-if-env-changed=NDK_HOME");
    println!("cargo:rerun-if-env-changed=ANDROID_PLATFORM");
    println!("cargo:rerun-if-env-changed=CARGO_NDK_PLATFORM");
    println!("cargo:rerun-if-env-changed=CARGO_NDK_ANDROID_PLATFORM");
    println!("cargo:rerun-if-env-changed=CARGO_NDK_SYSROOT_PATH");
    println!("cargo:rerun-if-env-changed=CARGO_NDK_SYSROOT_LIBS_PATH");
    if native_requested && !native_tuning {
        println!(
            "cargo:warning=Ignoring -C target-cpu=native for cross-compiled libvmaf target {target}"
        );
    } else if !compiler_tuning_args.is_empty() {
        println!(
            "cargo:warning=Applying Rust target tuning to libvmaf C/C++: {}",
            compiler_tuning_args.join(" ")
        );
    }

    let meson = env::var_os("MESON").unwrap_or_else(|| "meson".into());
    let ninja = env::var_os("NINJA").unwrap_or_else(|| "ninja".into());
    require_build_tool(&meson, "Meson", "MESON")?;
    require_build_tool(&ninja, "Ninja", "NINJA")?;
    if is_x86 {
        require_nasm()?;
    }

    let asm_enabled = is_x86 || env::var_os("CARGO_FEATURE_ASM").is_some();
    let avx512_enabled = asm_enabled && is_x86;
    remove_dir_if_exists(&build_dir)?;
    remove_dir_if_exists(&install_dir)?;

    let mut setup = Command::new(&meson);
    setup
        .arg("setup")
        .arg(&build_dir)
        .arg(&source_dir)
        .arg(format!("--prefix={}", install_dir.display()))
        .arg("--libdir=lib")
        .arg("--default-library=static")
        .arg("--buildtype=release")
        .arg("-Db_ndebug=if-release")
        .arg("-Denable_tests=false")
        .arg("-Denable_docs=false")
        .arg("-Denable_tools=false")
        .arg("-Denable_cuda=false")
        .arg("-Denable_nvtx=false")
        .arg(feature_option("built-in-models", "built_in_models"))
        .arg(format!("-Denable_asm={asm_enabled}"))
        .arg(format!("-Denable_avx512={avx512_enabled}"))
        .arg(feature_option("float", "enable_float"));

    if !compiler_tuning_args.is_empty() {
        setup.arg("--native-file").arg(generate_native_tuning_file(
            &out_dir,
            &compiler_tuning_args,
        )?);
    }

    let cross_file = match env::var_os("VMAF_MESON_CROSS_FILE") {
        Some(path) => {
            println!("cargo:rerun-if-changed={}", PathBuf::from(&path).display());
            Some(PathBuf::from(path))
        }
        None => generate_mobile_cross_file(&target, &target_arch, &target_os, &out_dir)?,
    };
    if let Some(cross_file) = cross_file {
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

    match (target_os.as_str(), target_env.as_str()) {
        ("macos" | "ios", _) => println!("cargo:rustc-link-lib=c++"),
        ("android", _) => {
            if let Some(path) = env::var_os("CARGO_NDK_SYSROOT_LIBS_PATH") {
                println!(
                    "cargo:rustc-link-search=native={}",
                    PathBuf::from(path).display()
                );
            }
            println!("cargo:rustc-link-lib=c++_shared");
        }
        ("windows", "msvc") => {}
        ("windows", _) => println!("cargo:rustc-link-lib=stdc++"),
        _ => println!("cargo:rustc-link-lib=stdc++"),
    }

    if target_os == "android" {
        println!("cargo:rustc-link-lib=m");
        println!("cargo:rustc-link-lib=dl");
    } else if matches!(target_os.as_str(), "linux" | "freebsd") {
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

fn target_cpu_from_rustflags(encoded_rustflags: &str) -> Option<&str> {
    let arguments = encoded_rustflags.split('\x1f').collect::<Vec<_>>();
    let mut target_cpu = None;
    let mut index = 0;
    while index < arguments.len() {
        let argument = arguments[index];
        let codegen_option = if matches!(argument, "-C" | "--codegen") {
            index += 1;
            arguments.get(index).copied()
        } else if let Some(option) = argument.strip_prefix("--codegen=") {
            Some(option)
        } else {
            argument.strip_prefix("-C")
        };
        if let Some(value) = codegen_option.and_then(|option| option.strip_prefix("target-cpu=")) {
            target_cpu = Some(value);
        }
        index += 1;
    }
    target_cpu
}

fn compiler_tuning_args(
    target_arch: &str,
    is_msvc: bool,
    native_tuning: bool,
    target_features: &str,
) -> Vec<&'static str> {
    if is_msvc {
        return msvc_arch_arg(target_features).into_iter().collect();
    }
    if native_tuning {
        return if matches!(target_arch, "aarch64" | "arm") {
            vec!["-mcpu=native"]
        } else {
            vec!["-march=native", "-mtune=native"]
        };
    }
    if !matches!(target_arch, "x86" | "x86_64") {
        return Vec::new();
    }

    const X86_FEATURE_FLAGS: &[(&str, &str)] = &[
        ("avx", "-mavx"),
        ("avx2", "-mavx2"),
        ("fma", "-mfma"),
        ("avx512f", "-mavx512f"),
        ("avx512bw", "-mavx512bw"),
        ("avx512cd", "-mavx512cd"),
        ("avx512dq", "-mavx512dq"),
        ("avx512vl", "-mavx512vl"),
    ];
    let features = target_features.split(',').collect::<Vec<_>>();
    X86_FEATURE_FLAGS
        .iter()
        .filter_map(|(feature, flag)| features.contains(feature).then_some(*flag))
        .collect()
}

fn msvc_arch_arg(target_features: &str) -> Option<&'static str> {
    let features = target_features.split(',').collect::<Vec<_>>();
    const REQUIRED_AVX512_FEATURES: &[&str] = &[
        "avx2", "avx512f", "avx512bw", "avx512cd", "avx512dq", "avx512vl", "bmi1", "bmi2", "fma",
    ];
    const REQUIRED_AVX2_FEATURES: &[&str] = &["avx2", "bmi1", "bmi2", "fma"];
    if REQUIRED_AVX512_FEATURES
        .iter()
        .all(|feature| features.contains(feature))
    {
        Some("/arch:AVX512")
    } else if REQUIRED_AVX2_FEATURES
        .iter()
        .all(|feature| features.contains(feature))
    {
        Some("/arch:AVX2")
    } else if features.contains(&"avx") {
        Some("/arch:AVX")
    } else {
        None
    }
}

fn generate_native_tuning_file(
    out_dir: &Path,
    compiler_args: &[&str],
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let arguments = compiler_args
        .iter()
        .map(|argument| meson_string(argument))
        .collect::<Vec<_>>()
        .join(", ");
    let contents =
        format!("[built-in options]\nc_args = [{arguments}]\ncpp_args = [{arguments}]\n");
    let path = out_dir.join("meson-native-tuning.ini");
    fs::write(&path, contents)?;
    Ok(path)
}

fn remove_dir_if_exists(path: &Path) -> Result<(), std::io::Error> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn generate_mobile_cross_file(
    target: &str,
    target_arch: &str,
    target_os: &str,
    out_dir: &Path,
) -> Result<Option<PathBuf>, Box<dyn std::error::Error>> {
    let contents = match target_os {
        "ios" => ios_cross_file(target, target_arch)?,
        "android" => android_cross_file(target, target_arch)?,
        _ => return Ok(None),
    };

    let path = out_dir.join(format!("meson-{target}.ini"));
    fs::write(&path, contents)?;
    Ok(Some(path))
}

fn ios_cross_file(target: &str, target_arch: &str) -> Result<String, Box<dyn std::error::Error>> {
    let simulator = target.ends_with("-sim") || target_arch == "x86_64";
    let sdk = if simulator {
        "iphonesimulator"
    } else {
        "iphoneos"
    };
    let deployment_target =
        env::var("IPHONEOS_DEPLOYMENT_TARGET").unwrap_or_else(|_| "12.0".into());
    let clang_arch = match target_arch {
        "aarch64" => "arm64",
        "x86_64" => "x86_64",
        _ => return Err(format!("Unsupported iOS architecture: {target_arch}").into()),
    };
    let clang_target = format!(
        "{clang_arch}-apple-ios{deployment_target}{}",
        if simulator { "-simulator" } else { "" }
    );
    let sdk_path = command_stdout("xcrun", &["--sdk", sdk, "--show-sdk-path"])?;
    let clang = command_stdout("xcrun", &["--sdk", sdk, "--find", "clang"])?;
    let clangxx = command_stdout("xcrun", &["--sdk", sdk, "--find", "clang++"])?;
    let ar = command_stdout("xcrun", &["--sdk", sdk, "--find", "ar"])?;
    let strip = command_stdout("xcrun", &["--sdk", sdk, "--find", "strip"])?;
    let compiler_args = [
        "-target".to_string(),
        clang_target,
        "-isysroot".to_string(),
        sdk_path,
    ];

    Ok(format!(
        "[binaries]\nc = {}\ncpp = {}\nar = {}\nstrip = {}\n\n[host_machine]\nsystem = 'darwin'\ncpu_family = {}\ncpu = {}\nendian = 'little'\n\n[properties]\nneeds_exe_wrapper = true\n",
        meson_command(&clang, &compiler_args),
        meson_command(&clangxx, &compiler_args),
        meson_command(&ar, &[]),
        meson_command(&strip, &[]),
        meson_string(if target_arch == "aarch64" { "aarch64" } else { "x86_64" }),
        meson_string(clang_arch),
    ))
}

fn android_cross_file(
    target: &str,
    target_arch: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let sysroot = android_sysroot()?;
    let prebuilt = sysroot
        .parent()
        .ok_or("Android NDK sysroot has no parent directory")?;
    let bin = prebuilt.join("bin");
    let api_level = android_api_level();
    let clang_target = match target {
        "armv7-linux-androideabi" => "armv7a-linux-androideabi",
        _ => target,
    };
    let target_arg = format!("--target={clang_target}{api_level}");
    let sysroot_arg = format!("--sysroot={}", sysroot.display());
    let compiler_args = [target_arg, sysroot_arg];
    let (cpu_family, cpu) = match target_arch {
        "aarch64" => ("aarch64", "aarch64"),
        "arm" => ("arm", "armv7"),
        "x86_64" => ("x86_64", "x86_64"),
        "x86" => ("x86", "i686"),
        _ => return Err(format!("Unsupported Android architecture: {target_arch}").into()),
    };

    Ok(format!(
        "[binaries]\nc = {}\ncpp = {}\nar = {}\nstrip = {}\n\n[host_machine]\nsystem = 'android'\ncpu_family = {}\ncpu = {}\nendian = 'little'\n\n[properties]\nneeds_exe_wrapper = true\n",
        meson_command(&bin.join("clang").to_string_lossy(), &compiler_args),
        meson_command(&bin.join("clang++").to_string_lossy(), &compiler_args),
        meson_command(&bin.join("llvm-ar").to_string_lossy(), &[]),
        meson_command(&bin.join("llvm-strip").to_string_lossy(), &[]),
        meson_string(cpu_family),
        meson_string(cpu),
    ))
}

fn android_sysroot() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(path) = env::var_os("CARGO_NDK_SYSROOT_PATH") {
        return Ok(PathBuf::from(path));
    }

    let ndk_root = [
        "ANDROID_NDK_HOME",
        "ANDROID_NDK_ROOT",
        "ANDROID_NDK_PATH",
        "NDK_HOME",
    ]
    .iter()
    .find_map(env::var_os)
    .map(PathBuf::from)
    .ok_or("Android NDK not found. Set ANDROID_NDK_HOME or build through cargo-ndk.")?;
    let prebuilt_root = ndk_root.join("toolchains/llvm/prebuilt");
    let prebuilt = fs::read_dir(&prebuilt_root)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| path.join("bin/clang").is_file())
        .ok_or_else(|| {
            format!(
                "No usable LLVM toolchain found under {}",
                prebuilt_root.display()
            )
        })?;
    Ok(prebuilt.join("sysroot"))
}

fn android_api_level() -> String {
    [
        "CARGO_NDK_ANDROID_PLATFORM",
        "ANDROID_PLATFORM",
        "CARGO_NDK_PLATFORM",
    ]
    .iter()
    .find_map(|name| env::var(name).ok())
    .map(|value| value.trim_start_matches("android-").to_string())
    .filter(|value| !value.is_empty() && value.chars().all(|c| c.is_ascii_digit()))
    .unwrap_or_else(|| "21".into())
}

fn command_stdout(program: &str, args: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let output = Command::new(program).args(args).output()?;
    if !output.status.success() {
        return Err(format!(
            "{} {} failed with status {}",
            program,
            args.join(" "),
            output.status
        )
        .into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn meson_command(executable: &str, args: &[String]) -> String {
    let values = std::iter::once(executable)
        .chain(args.iter().map(String::as_str))
        .map(meson_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn meson_string(value: &str) -> String {
    format!("'{}'", value.replace('\\', "\\\\").replace('\'', "\\'"))
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
        Ok(output) if output.status.success() => {
            let version = parse_nasm_version(&output.stdout)
                .ok_or("Could not parse the installed NASM version")?;
            if version < (2, 14) {
                return Err(format!(
                    "NASM 2.14 or later is required for AVX-512 builds; found {}.{}",
                    version.0, version.1
                )
                .into());
            }
            Ok(())
        }
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

fn parse_nasm_version(output: &[u8]) -> Option<(u32, u32)> {
    String::from_utf8_lossy(output)
        .split_whitespace()
        .find_map(|token| {
            let mut components = token.split('.');
            let major = components.next()?.parse().ok()?;
            let minor = components.next()?.parse().ok()?;
            Some((major, minor))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn target_cpu_parser_accepts_cargo_flag_forms_and_last_value_wins() {
        assert_eq!(
            target_cpu_from_rustflags("-C\x1ftarget-cpu=native"),
            Some("native")
        );
        assert_eq!(
            target_cpu_from_rustflags("-Ctarget-cpu=x86-64-v3"),
            Some("x86-64-v3")
        );
        assert_eq!(
            target_cpu_from_rustflags("--codegen=target-cpu=native"),
            Some("native")
        );
        assert_eq!(
            target_cpu_from_rustflags("-Ctarget-cpu=native\x1f-C\x1ftarget-cpu=x86-64-v2"),
            Some("x86-64-v2")
        );
    }

    #[test]
    fn native_tuning_uses_architecture_appropriate_compiler_flags() {
        assert_eq!(
            compiler_tuning_args("x86_64", false, true, "avx2,fma"),
            ["-march=native", "-mtune=native"]
        );
        assert_eq!(
            compiler_tuning_args("aarch64", false, true, "neon"),
            ["-mcpu=native"]
        );
    }

    #[test]
    fn explicit_x86_features_are_forwarded_to_gnu_style_compilers() {
        assert_eq!(
            compiler_tuning_args("x86_64", false, false, "avx,avx2,avx512bw,avx512f,fma,sse2"),
            ["-mavx", "-mavx2", "-mfma", "-mavx512f", "-mavx512bw"]
        );
    }

    #[test]
    fn msvc_uses_highest_resolved_x86_architecture() {
        assert_eq!(msvc_arch_arg("avx,avx2,bmi1,bmi2,fma"), Some("/arch:AVX2"));
        assert_eq!(
            msvc_arch_arg("avx,avx2,avx512bw,avx512cd,avx512dq,avx512f,avx512vl,bmi1,bmi2,fma"),
            Some("/arch:AVX512")
        );
        assert_eq!(msvc_arch_arg("avx,avx2,fma"), Some("/arch:AVX"));
        assert_eq!(
            msvc_arch_arg("avx,avx2,avx512f,bmi1,bmi2,fma"),
            Some("/arch:AVX2")
        );
        assert_eq!(msvc_arch_arg("sse,sse2"), None);
    }

    #[test]
    fn nasm_version_parser_reads_release_components() {
        assert_eq!(
            parse_nasm_version(b"NASM version 2.16.03 compiled on Jan 1 2026"),
            Some((2, 16))
        );
        assert_eq!(parse_nasm_version(b"NASM version 2.13.02"), Some((2, 13)));
        assert_eq!(parse_nasm_version(b"not a version"), None);
    }
}
