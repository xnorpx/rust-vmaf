#!/usr/bin/env python3
"""Exit with code 1 when the vendored VMAF revision is not upstream HEAD."""

import json
import re
import sys
import urllib.request
from pathlib import Path


def latest_commit():
    request = urllib.request.Request(
        "https://api.github.com/repos/Netflix/vmaf/commits/master",
        headers={"User-Agent": "vmaf-head-sys-version-check"},
    )
    with urllib.request.urlopen(request) as response:
        return json.loads(response.read().decode())["sha"]


def vendored_commit():
    version_file = Path(__file__).parent / "vendored" / "VMAF_VERSION"
    if not version_file.exists():
        return None
    match = re.search(r"^commit:\s*([a-fA-F0-9]+)", version_file.read_text(), re.MULTILINE)
    return match.group(1) if match else None


def main():
    vendored = vendored_commit()
    if not vendored:
        print("ERROR: no vendored VMAF version found")
        return 1

    latest = latest_commit()
    print(f"Vendored: {vendored[:12]}")
    print(f"Latest:   {latest[:12]}")
    if vendored == latest:
        print("Up to date")
        return 0

    print("Update needed; run: python vendor_vmaf.py")
    return 1


if __name__ == "__main__":
    sys.exit(main())