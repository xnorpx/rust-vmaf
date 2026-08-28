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
- NASM 2.14 or later on x86 and x86_64 platforms

CUDA is intentionally disabled. This crate binds the portable CPU API and does not expose `libvmaf_cuda.h`.

### Cargo features

- `built-in-models` - compile standard VMAF models into libvmaf (default)
- `asm` - enable optional architecture-specific optimizations (default)
- `float` - compile floating-point feature extractors

On x86 and x86_64, NASM support and AVX2/AVX-512 kernels are always built, including with `--no-default-features` and MSVC. VMAF uses CPUID and operating-system state checks at runtime, so optimized instructions execute only on supported machines. Portable builds therefore include the fastest available VMAF kernels without raising the binary's baseline CPU requirement.

MSVC builds use a private Windows-native pthread translation layer. The optimized x86 sources use the same runtime dispatch as Windows GNU builds.

### CPU tuning

The native library is compiled with Meson's release optimization level. Rust target settings are also propagated to the C and C++ compilers:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

For a host build, `target-cpu=native` adds `-march=native -mtune=native` on x86 GCC/Clang, `-mcpu=native` on ARM GCC/Clang, or the highest compatible resolved `/arch:AVX*` level on MSVC. Explicit Rust x86 target features such as `+avx2`, `+fma`, and `+avx512f` are forwarded to GCC/Clang as matching compiler flags. MSVC selects an aggregate `/arch` level only when Rust enables every feature that compiler level may assume.

Native and explicit target-feature tuning may raise the application's baseline CPU requirement. C/C++ tuning is intentionally ignored for cross-compilation; runtime-dispatched VMAF kernels remain enabled in cross-built x86 binaries.

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

CI also builds and runs the test suite for both `x86_64-pc-windows-gnu` and `x86_64-pc-windows-msvc`.

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

The update script applies the ordered patches under `patches/` after copying upstream VMAF. Patch failures stop the update so upstream changes cannot silently drop local platform fixes.

The current patch set keeps upstream changes individually attributable:

- `ya_getopt.patch` - [Netflix/VMAF#1410](https://github.com/Netflix/vmaf/pull/1410)
- `msvc_no_vla.patch` - [Netflix/VMAF#1428](https://github.com/Netflix/vmaf/pull/1428)
- `avx2_simd_portability.patch` - [Netflix/VMAF#1475](https://github.com/Netflix/vmaf/pull/1475)
- `vif_void_pointer.patch` - [Netflix/VMAF#1476](https://github.com/Netflix/vmaf/pull/1476)
- `msvc_pthread.patch` and `msvc_simd_build.patch` - private Windows threading and SIMD build integration
- `avx2_fma_dispatch.patch` - require FMA before selecting VMAF's AVX2/FMA kernels
- `vif_size_overflow.patch` - checked copy-size arithmetic

Regenerate bindings without downloading VMAF:

```bash
python vendor_vmaf.py --bindings-only
```

## License

The Rust bindings are licensed under either [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option.

The vendored VMAF source is licensed under BSD-2-Clause-Patent. See [NOTICE](NOTICE) and `vendored/vmaf/LICENSE`.