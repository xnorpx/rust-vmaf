# vmaf-head-sys

Rust FFI bindings to [Netflix VMAF](https://github.com/Netflix/vmaf) with vendored source code.

## Overview

This crate provides low-level bindings to libvmaf, built from vendored source. It tracks VMAF master as closely as possible with frequent updates.

### Features

- **Vendored source** - libvmaf is compiled from source rather than discovered through pkg-config
- **Public CPU C API** - bindings cover features, models, pictures, and scoring
- **Static linking** - libvmaf is linked into the final binary
- **Built-in models** - standard VMAF models are available by default

### Requirements

Building requires:

- Meson and Ninja
- A C and C++ compiler
- `xxd` when `built-in-models` is enabled
- NASM on x86 and x86_64 platforms

CUDA is intentionally disabled. This crate binds the portable CPU API and does not expose `libvmaf_cuda.h`.

### Cargo features

- `built-in-models` - compile standard VMAF models into libvmaf (default)
- `asm` - enable optional architecture-specific optimizations (default)
- `float` - compile floating-point feature extractors

On x86 and x86_64, NASM support and AVX2 kernels are always built, including with `--no-default-features`. VMAF still uses CPUID runtime dispatch, so AVX2 instructions execute only when both the CPU and operating system support them. This avoids illegal-instruction crashes on older x86 systems.

## Usage

```toml
[dependencies]
vmaf-head-sys = "0.1"
```

All libvmaf calls are unsafe raw FFI calls. Applications will usually want a safe wrapper around this crate.

## Cross-compilation

iOS device/simulator and Android cross files are generated automatically from Cargo's target and the active SDK/NDK. Set `IPHONEOS_DEPLOYMENT_TARGET` to override the default iOS 12.0 deployment target. For Android, set `ANDROID_NDK_HOME` and optionally `ANDROID_PLATFORM` (default API 21), or build through `cargo-ndk`.

Set `VMAF_MESON_CROSS_FILE` to override the generated file for a custom toolchain. Set `MESON` or `NINJA` to override either build-tool executable.

CI compiles and links these mobile targets:

- `aarch64-apple-ios`
- `aarch64-apple-ios-sim`
- `x86_64-apple-ios`
- `aarch64-linux-android` (`arm64-v8a`)
- `armv7-linux-androideabi` (`armeabi-v7a`)
- `x86_64-linux-android`
- `i686-linux-android` (`x86`)

For Android final binaries, `c++_shared` must be packaged with the application. With `cargo-ndk` and NDK r29, use:

```bash
cargo ndk --platform 24 --target arm64-v8a \
	--link-builtins --link-libcxx-shared build
```

## Vendored Version

The vendored VMAF source is tracked in `vendored/VMAF_VERSION`. Run the update script to sync with upstream:

```bash
python vendor_vmaf.py
```

Regenerate bindings without downloading VMAF:

```bash
python vendor_vmaf.py --bindings-only
```

## License

The Rust bindings are licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

The vendored VMAF source is licensed under BSD-2-Clause-Patent. See [NOTICE](NOTICE) and `vendored/vmaf/LICENSE`.