---
name: vmaf-vendor-release
description: 'Update and release vmaf-head-sys when check_vmaf_version.py or the Check VMAF Version workflow fails because vendored Netflix VMAF is stale. Use when: the VMAF version check reports Update needed, vendored VMAF is behind master, VMAF HEAD should be vendored, Rust bindings need regeneration, or a new vmaf-head-sys GitHub/crates.io release should be prepared.'
argument-hint: '[prepare|release] [patch|minor|major]'
user-invocable: true
disable-model-invocation: true
---

# Vendor And Release VMAF

Update the vendored VMAF snapshot, regenerate bindings, assess the public API diff, validate the crate, and optionally release it.

## Manual Invocation

Run this skill manually after inspecting a failed Check VMAF Version workflow:

```text
/vmaf-vendor-release prepare
/vmaf-vendor-release release
```

Never invoke this skill automatically from a CI failure. If no mode is supplied, use `prepare` and stop before all commit, push, tag, GitHub release, and crates.io publication steps. Even in `release` mode, retain both explicit approval checkpoints below.

## Safety Contract

- Treat the version check as stale only when it prints `Update needed` and the vendored and latest SHAs differ. Network failures, GitHub API failures, rate limits, and malformed version files are not update signals.
- Preserve all user changes. Never reset, revert, clean, or overwrite unrelated work.
- Require a clean worktree before starting a release. If unrelated changes exist, report them and ask how to isolate the release before continuing.
- Do not commit, tag, or push without explicit approval after presenting the complete diff and validation results.
- Do not run `cargo publish` without a second explicit approval after the GitHub release succeeds.
- Never force-push, move an existing release tag, or replace a published crate version.
- Never request or transmit registry tokens through chat. If authentication is needed, have the user enter secrets directly in the terminal.

## 1. Diagnose The Failure

1. Work from the repository root containing `Cargo.toml`, `vendor_vmaf.py`, and `vendored/VMAF_VERSION`.
2. Inspect `git status --short` and the current branch. Do not proceed over unrelated changes.
3. Inspect the failed workflow log when available. Distinguish an actual stale revision from infrastructure failure.
4. Run:

   ```bash
   python3 check_vmaf_version.py
   ```

5. If it reports `Up to date`, stop unless the user explicitly requested a forced refresh.
6. Record before changing anything:
   - Current crate version from `Cargo.toml`
   - Old VMAF SHA from `vendored/VMAF_VERSION`
   - Current branch and upstream remote
7. Verify required tools are available: `python3`, `git`, `cargo`, `bindgen`, `meson`, `ninja`, `xxd`, and NASM on x86 platforms.

## 2. Update The Vendored Snapshot

1. Update from upstream GitHub, not an arbitrary local checkout:

   ```bash
   python3 vendor_vmaf.py --force
   ```

2. Confirm the update immediately:

   ```bash
   python3 check_vmaf_version.py
   ```

3. Verify that `vendored/VMAF_VERSION` now records the upstream HEAD SHA and date.
4. Review the update before making any version decision:

   ```bash
   git diff --stat
   git diff -- vendored/VMAF_VERSION
   git diff -- src/bindings.rs
   git diff -- vendored/vmaf/libvmaf/include
   git diff -- vendored/vmaf/libvmaf/meson.build
   ```

5. If vendoring produced no tracked diff, stop and report that no release is needed.
6. Check that generated bindings still expose the complete non-CUDA public CPU API and API version constants. Do not hand-edit `src/bindings.rs`; fix `vendor_vmaf.py` and regenerate instead.

## 3. Recommend A Version

Analyze the generated Rust binding diff and recommend a SemVer bump before editing `Cargo.toml`:

- **Patch**: no public Rust FFI additions, removals, signature changes, constant changes, or layout changes; only implementation, model, build, or vendored-source updates.
- **Minor for 0.x**: any public FFI addition or breaking change while the crate is pre-1.0.
- **Minor for 1.x+**: backward-compatible public FFI additions.
- **Major for 1.x+**: removals, renamed symbols, changed signatures, changed constants with API meaning, or changed public type/layout contracts.

Present the recommendation, evidence from `src/bindings.rs`, and proposed new version. Ask for approval before changing the version. If the invocation explicitly supplied `patch`, `minor`, or `major`, still report conflicts between that choice and the observed API diff.

After approval:

1. Update only the package version in `Cargo.toml`.
2. Let Cargo update `Cargo.lock` if it is tracked by this repository.
3. Do not alter dependency versions unless the VMAF update requires it.

## 4. Run The Release Gate

Run every check and stop on the first failure:

```bash
python3 -m py_compile vendor_vmaf.py check_vmaf_version.py
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --no-default-features
cargo test --all-features
DOCS_RS=1 RUSTDOCFLAGS='-D warnings' cargo doc --all-features --no-deps
actionlint .github/workflows/*.yml
python3 check_vmaf_version.py
cargo package --allow-dirty
cargo publish --dry-run --allow-dirty
```

Also verify:

- The packaged crate remains below crates.io's compressed size limit.
- `cargo package --list --allow-dirty` includes `vendored/VMAF_VERSION`, required VMAF sources/models, generated bindings, and test fixtures.
- JPEG integration tests score every committed resolution when `built-in-models` is enabled.
- No unexpected generated or untracked files are included.

If a check fails, repair only update-related failures and rerun the failed check followed by the complete gate. Do not release with skipped checks.

## 5. Approval Checkpoint

Before any git write or network publication, present:

- Old and new VMAF SHAs and dates
- Old and proposed crate versions
- Upstream and generated-binding change summary
- SemVer rationale
- Full validation results
- Package file count and compressed size
- Exact `git status --short` and `git diff --stat`
- Proposed commit message and tag

Use the commit message `chore: update VMAF to <short-sha>` and tag `v<crate-version>` unless the user requests another convention.

Ask for explicit approval to commit, push the default branch, create the annotated tag, and push the tag.

## 6. Create The GitHub Release

After approval:

1. Stage only intended release files. Review `git diff --cached` before committing; never use a broad staging command when unrelated paths exist.
2. Commit the approved update.
3. Push the default branch without force.
4. Wait for the branch CI workflow to succeed. With GitHub CLI available, locate the matching run and use `gh run watch <run-id> --exit-status`.
5. Only after branch CI succeeds, create an annotated `v<crate-version>` tag and push it without force.
6. The existing `.github/workflows/release.yml` workflow creates the GitHub release from that tag.
7. Wait for the release workflow and verify it with:

   ```bash
   gh release view v<crate-version>
   ```

If `gh` is unavailable, stop after each push and ask the user to confirm the corresponding workflow succeeded before continuing. Never create a tag when branch CI is failing.

## 7. Publish To crates.io

After the GitHub release is visible and successful:

1. Show the exact package/version that will be published.
2. Ask for a second explicit approval.
3. Run from the clean tagged revision:

   ```bash
   cargo publish
   ```

4. Verify the version appears on crates.io. If publication reports that the version already exists, verify it and stop; do not bump or republish automatically.

## 8. Final Report

Report:

- Released crate version and tag
- Old and new VMAF SHAs
- Public binding/API impact
- Checks executed and their results
- GitHub release URL
- crates.io publication status and URL
- Any manual follow-up still required

If invoked in `prepare` mode, stop after the approval checkpoint with no commit, tag, push, GitHub release, or crates.io publication.