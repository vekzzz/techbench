"""Rebuilds crates/toolbox-core/data/oui.tsv from the IEEE OUI registry (MA-L assignments).

Usage: python scripts/update-oui.py
The result is compiled into the toolbox core, so LAN Scan's vendor lookup works offline.
"""
import re
import urllib.request
from pathlib import Path

SOURCE = "https://standards-oui.ieee.org/oui/oui.txt"
TARGET = Path(__file__).resolve().parent.parent / "crates" / "toolbox-core" / "data" / "oui.tsv"

request = urllib.request.Request(SOURCE, headers={"User-Agent": "it-tools-oui-update"})
text = urllib.request.urlopen(request, timeout=60).read().decode("utf-8", errors="replace")

entries = {}
for line in text.splitlines():
    match = re.match(r"^([0-9A-F]{2})-([0-9A-F]{2})-([0-9A-F]{2})\s+\(hex\)\s+(.+?)\s*$", line)
    if match:
        prefix = "".join(match.group(1, 2, 3))
        name = match.group(4)
        # Private registrations are listed under this name; they tell the Technician nothing.
        if name.lower() != "private":
            entries[prefix] = name

TARGET.write_text("".join(f"{prefix}\t{entries[prefix]}\n" for prefix in sorted(entries)), encoding="utf-8", newline="\n")
print(f"{len(entries)} vendors written to {TARGET}")
