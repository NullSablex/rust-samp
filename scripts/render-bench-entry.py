#!/usr/bin/env python3
"""Renders `bench-entry.json` as a Markdown table for GitHub Step Summary.

Output goes to stdout. Format:
    ### Bench data (this run)
    | Benchmark | Mean (ns) | 95% CI | Width |
    | --- | ---: | --- | ---: |
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ENTRY = Path("bench-entry.json")


def fmt_ns(value: float) -> str:
    if value < 1_000:
        return f"{value:.1f}"
    if value < 1_000_000:
        return f"{value / 1_000:.2f}k"
    return f"{value / 1_000_000:.2f}M"


def main() -> None:
    if not ENTRY.is_file():
        sys.exit(f"error: {ENTRY} not found")
    entry = json.loads(ENTRY.read_text())
    benches = entry.get("benchmarks", {})
    if not benches:
        print("_No benchmark data extracted._")
        return

    print(f"### Bench data — `{entry.get('version', '?')}` (`{entry.get('commit', '?')[:7]}`)")
    print()
    print("| Benchmark | Mean (ns) | 95% CI lower | 95% CI upper | CI width |")
    print("| --- | ---: | ---: | ---: | ---: |")
    for name in sorted(benches):
        b = benches[name]
        mean = b["mean_ns"]
        lo = b["lower_ns"]
        hi = b["upper_ns"]
        width_pct = ((hi - lo) / mean * 100.0) if mean else 0.0
        print(
            f"| `{name}` | {fmt_ns(mean)} | {fmt_ns(lo)} | {fmt_ns(hi)} | {width_pct:.1f}% |"
        )


if __name__ == "__main__":
    main()
