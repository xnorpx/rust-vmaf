#!/usr/bin/env python3
"""Vendor VMAF HEAD, apply local patches, and generate Rust bindings."""

import argparse
import json
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path


REPOSITORY = "https://github.com/Netflix/vmaf.git"


def run_command(command, cwd=None):
    print(f"Running: {' '.join(map(str, command))}")
    result = subprocess.run(command, cwd=cwd, capture_output=True, text=True)
    if result.returncode != 0:
        print(result.stdout)
        print(result.stderr, file=sys.stderr)
        raise RuntimeError(f"Command failed with code {result.returncode}")
    return result.stdout.strip()


def latest_commit_info():
    request = urllib.request.Request(
        "https://api.github.com/repos/Netflix/vmaf/commits/master",
        headers={
            "Accept": "application/vnd.github.v3+json",
            "User-Agent": "vmaf-head-sys-vendor-script",
        },
    )
    with urllib.request.urlopen(request) as response:
        data = json.loads(response.read().decode())
    return {
        "sha": data["sha"],
        "date": data["commit"]["committer"]["date"][:10],
        "message": data["commit"]["message"].splitlines()[0],
    }


def current_commit(vendored_dir):
    version_file = vendored_dir / "VMAF_VERSION"
    if not version_file.exists():
        return None
    match = re.search(r"^commit:\s*([a-fA-F0-9]+)", version_file.read_text(), re.MULTILINE)
    return match.group(1) if match else None


def copy_source(source_dir, target_dir):
    if target_dir.exists():
        shutil.rmtree(target_dir)
    target_dir.mkdir(parents=True)

    for directory in ("libvmaf", "model"):
        shutil.copytree(source_dir / directory, target_dir / directory)
    shutil.copy2(source_dir / "LICENSE", target_dir / "LICENSE")


def apply_patches(project_dir, vmaf_dir, patches_dir):
    patch_root = vmaf_dir.relative_to(project_dir)
    for patch in sorted(patches_dir.glob("*.patch")):
        print(f"Applying vendor patch: {patch.name}")
        run_command(
            [
                "git",
                "apply",
                "--verbose",
                "--whitespace=error-all",
                f"--directory={patch_root}",
                str(patch.resolve()),
            ],
            cwd=project_dir,
        )


def write_version_file(vendored_dir, commit_info):
    content = f"""# VMAF Vendor Information
#
# This file tracks the exact version of the vendored VMAF source.

source: https://github.com/Netflix/vmaf
commit: {commit_info['sha']}
date: {commit_info['date']}
branch: master

# To update, run:
# python vendor_vmaf.py
"""
    (vendored_dir / "VMAF_VERSION").write_text(content)


def generate_bindings(vmaf_dir, output_dir):
    include_dir = vmaf_dir / "libvmaf" / "include"
    meson_build = (vmaf_dir / "libvmaf" / "meson.build").read_text()
    version = re.search(
        r"vmaf_soname_version\s*=\s*'(\d+)\.(\d+)\.(\d+)'", meson_build
    )
    if not version:
        raise RuntimeError("Could not determine the VMAF API version")

    headers = [
        "libvmaf/feature.h",
        "libvmaf/model.h",
        "libvmaf/picture.h",
        "libvmaf/libvmaf.h",
    ]

    with tempfile.NamedTemporaryFile(mode="w", suffix=".h", delete=False) as wrapper:
        for component, value in zip(("MAJOR", "MINOR", "PATCH"), version.groups()):
            wrapper.write(f"#define VMAF_API_VERSION_{component} {value}\n")
        for header in headers:
            wrapper.write(f'#include "{header}"\n')
        wrapper_path = Path(wrapper.name)

    try:
        output_dir.mkdir(parents=True, exist_ok=True)
        run_command(
            [
                "bindgen",
                str(wrapper_path),
                "--output",
                str(output_dir / "bindings.rs"),
                "--allowlist-function",
                "vmaf_.*",
                "--allowlist-type",
                "Vmaf.*",
                "--allowlist-var",
                "VMAF_.*",
                "--no-layout-tests",
                "--raw-line",
                "#![allow(rustdoc::broken_intra_doc_links)]",
                "--raw-line",
                "#![allow(rustdoc::bare_urls)]",
                "--",
                f"-I{include_dir}",
            ]
        )
    finally:
        wrapper_path.unlink(missing_ok=True)


def source_commit_info(source_dir):
    return {
        "sha": run_command(["git", "rev-parse", "HEAD"], cwd=source_dir),
        "date": run_command(["git", "log", "-1", "--format=%cs"], cwd=source_dir),
        "message": run_command(["git", "log", "-1", "--format=%s"], cwd=source_dir),
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bindings-only", action="store_true")
    parser.add_argument("--force", action="store_true")
    parser.add_argument(
        "--source",
        type=Path,
        help="Vendor an existing VMAF git checkout instead of cloning GitHub",
    )
    args = parser.parse_args()

    project_dir = Path(__file__).parent.resolve()
    vendored_dir = project_dir / "vendored"
    vmaf_dir = vendored_dir / "vmaf"

    if args.bindings_only:
        if not vmaf_dir.is_dir():
            parser.error("vendored source is missing; run without --bindings-only first")
        generate_bindings(vmaf_dir, project_dir / "src")
        return

    temporary_clone = None
    if args.source:
        source_dir = args.source.resolve()
        commit_info = source_commit_info(source_dir)
    else:
        commit_info = latest_commit_info()
        if current_commit(vendored_dir) == commit_info["sha"] and not args.force:
            print(f"Already up to date at {commit_info['sha'][:12]}")
            return
        temporary_clone = tempfile.TemporaryDirectory()
        source_dir = Path(temporary_clone.name) / "vmaf"
        run_command(["git", "clone", "--depth", "1", REPOSITORY, str(source_dir)])
        commit_info = source_commit_info(source_dir)

    print(f"Vendoring VMAF {commit_info['sha'][:12]}: {commit_info['message']}")
    copy_source(source_dir, vmaf_dir)
    apply_patches(project_dir, vmaf_dir, project_dir / "patches")
    write_version_file(vendored_dir, commit_info)
    generate_bindings(vmaf_dir, project_dir / "src")
    if temporary_clone:
        temporary_clone.cleanup()

    print("Updated patched VMAF source, vendored/VMAF_VERSION, and src/bindings.rs")


if __name__ == "__main__":
    main()