#!/usr/bin/env python3
"""Appends `bench-entry.json` to `bench-history.json` (array)."""

from __future__ import annotations

import json
import sys
from pathlib import Path

HISTORY = Path("bench-history.json")
ENTRY = Path("bench-entry.json")


def main() -> None:
    if not ENTRY.is_file():
        sys.exit(f"error: {ENTRY} not found")

    entry = json.loads(ENTRY.read_text())
    history = json.loads(HISTORY.read_text()) if HISTORY.is_file() else []
    if not isinstance(history, list):
        sys.exit(f"error: {HISTORY} is not a JSON array")

    history.append(entry)
    HISTORY.write_text(json.dumps(history, indent=2) + "\n")
    print(f"appended entry for version={entry.get('version')} ({len(history)} total)")


if __name__ == "__main__":
    main()
