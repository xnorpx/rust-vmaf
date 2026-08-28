# Repository Guidelines

## Purpose

This repository publishes `vmaf-head-sys`, raw Rust FFI bindings to a statically linked, vendored snapshot of Netflix VMAF HEAD. Keep changes focused on the low-level CPU API; CUDA is intentionally disabled and safe wrappers belong in a separate crate.

## Generated And Vendored Files

- Do not edit `src/bindings.rs` by hand. Change `vendor_vmaf.py`, then run `python3 vendor_vmaf.py --bindings-only`.
- Update `vendored/vmaf` only through `python3 vendor_vmaf.py`. Preserve upstream source, licenses, formatting, and model data byte-for-byte.
- `vendored/VMAF_VERSION` must identify the exact upstream commit. Verify it with `python3 check_vmaf_version.py`.
- Keep bindgen output target-independent. In particular, retain `--no-layout-tests` so committed bindings compile on 32-bit targets.

## Native Build Invariants

- `build.rs` owns Meson configuration, static linking, platform runtime libraries, and generated iOS/Android cross files.
- Meson and Ninja are required for native builds. `xxd` is required for built-in models.
- x86 and x86_64 builds compile NASM and AVX2 paths, including with `--no-default-features`; runtime CPUID dispatch must remain enabled. MSVC-target builds are the exception and use scalar kernels compiled with clang-cl.
- MSVC-target builds use the compatibility headers under `compat/msvc` for libvmaf's pthread and POSIX APIs; keep those shims outside the vendored source tree.
- AArch64 builds use VMAF's NEON paths when the `asm` feature is enabled.
- Preserve compile-and-link support for every target in `.github/workflows/ci.yml`, including iOS device/simulators and all four Android ABIs.
- Android uses NDK libc++ (`c++_shared`), not GNU `stdc++`. NDK r29 x86 links require Clang builtins through `cargo-ndk --link-builtins`.

## Validation

Run focused checks while iterating. Before finishing a change, run the applicable parts of this gate:

```bash
python3 -m py_compile vendor_vmaf.py check_vmaf_version.py
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --no-default-features
cargo test --all-features
DOCS_RS=1 RUSTDOCFLAGS='-D warnings' cargo doc --all-features --no-deps
actionlint .github/workflows/*.yml
cargo package --allow-dirty
```

Changes to bindings, native compilation, linking, or platform logic also require the relevant cross-target compile/link checks from `.github/workflows/ci.yml`.

## Releases

- Follow `RELEASE.md` and use the manual `/vmaf-vendor-release` skill after a genuine stale-version CI failure.
- Never trigger the vendor/release workflow automatically.
- Do not commit, tag, push, create a GitHub release, or run `cargo publish` unless the user explicitly requests that external action.
- Keep crates.io package contents and compressed size within its limits; maintenance scripts and GitHub metadata remain excluded from the crate package.