# Release Process

This document describes how to release `vmaf-head-sys`.

## Pre-release Checklist

1. Update the version in `Cargo.toml`.
2. Update the vendored VMAF snapshot with `python vendor_vmaf.py`.
3. Run the local checks:

   ```bash
   cargo test --all-features
   cargo clippy --all-targets --all-features -- -D warnings
   cargo publish --dry-run
   ```

## Creating a Release

Commit and push the version changes, then create an annotated tag matching the Cargo version:

```bash
git tag -a v0.1.0 -m "Release v0.1.0"
git push origin v0.1.0
```

The release workflow validates the tag against `Cargo.toml` and creates the GitHub release.

After the workflow succeeds, publish the crate:

```bash
cargo publish
```

## Versioning

This project follows semantic versioning. Because the crate tracks VMAF HEAD, any upstream public C API change that alters generated Rust bindings may require a breaking crate release.

## Troubleshooting

If crates.io rejects the package size, inspect its contents and compressed size:

```bash
cargo package --list
ls -lh target/package/vmaf-head-sys-*.crate
```

Only the source required to compile libvmaf should be vendored. Python tooling, test resources, and upstream workspace files are intentionally omitted.