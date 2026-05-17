#!/usr/bin/env python3
"""Builds the PR bench comment, comparing the current run against the
previous run's data carried inside the existing sticky comment.

The previous comment (if any) contains a hidden marker with the prior
`bench-entry.json` base64-encoded:

    <!--rs-bench-data:v1
    <base64 payload>
    -->

We extract that, decode, and produce a side-by-side comparison. The new
comment embeds the current bench-entry as the next iteration's "previous"
payload — so every PR run keeps `before` from the last run + `after` from
the new run, with no dependency on Actions cache.

Usage:
    build-bench-comment.py \\
        --current  bench-entry.json   \\
        --previous previous_comment.md \\
        --version  "5#abc1234"        \\
        --sha      abc1234567890      \\
        --run-url  https://...        \\
        --output   pr_comment.md
"""

from __future__ import annotations

import argparse
import base64
import json
import re
import sys
from pathlib import Path
from typing import Optional

MARKER_OPEN = "<!--rs-bench-data:v1"
MARKER_CLOSE = "-->"

# Thresholds (percent change) used to flag regressions / improvements.
THRESH_MAJOR = 10.0   # red flag
THRESH_MINOR = 5.0    # yellow flag


# ---------------------------------------------------------------------------
# Marker (de)serialization
# ---------------------------------------------------------------------------


def encode_marker(entry: dict) -> str:
    payload = base64.b64encode(json.dumps(entry, separators=(",", ":")).encode()).decode()
    # Wrap at 76 chars to keep diffs reviewable.
    chunks = [payload[i : i + 76] for i in range(0, len(payload), 76)]
    return f"{MARKER_OPEN}\n" + "\n".join(chunks) + f"\n{MARKER_CLOSE}"


def decode_marker(comment_body: str) -> Optional[dict]:
    if not comment_body:
        return None
    pattern = re.escape(MARKER_OPEN) + r"\s*(.*?)\s*" + re.escape(MARKER_CLOSE)
    match = re.search(pattern, comment_body, re.DOTALL)
    if not match:
        return None
    payload = re.sub(r"\s+", "", match.group(1))
    try:
        return json.loads(base64.b64decode(payload).decode())
    except (ValueError, json.JSONDecodeError):
        return None


# ---------------------------------------------------------------------------
# Number formatting
# ---------------------------------------------------------------------------


def format_time(ns: float) -> str:
    """Pretty-prints a duration in ns/µs/ms/s with 3 significant digits."""
    if ns is None:
        return "—"
    abs_ns = abs(ns)
    if abs_ns < 1e3:
        return f"{ns:.2f} ns"
    if abs_ns < 1e6:
        return f"{ns / 1e3:.2f} µs"
    if abs_ns < 1e9:
        return f"{ns / 1e6:.2f} ms"
    return f"{ns / 1e9:.2f} s"


def format_ci(lower_ns: float, upper_ns: float, mean_ns: float) -> str:
    """± half-width as a percentage of the mean (95% CI)."""
    if mean_ns == 0:
        return ""
    half_width = (upper_ns - lower_ns) / 2
    pct = (half_width / mean_ns) * 100
    return f"±{pct:.1f}%"


def format_delta(before_ns: float, after_ns: float) -> tuple[str, str]:
    """Returns (delta_text, icon). Negative = faster, positive = slower."""
    if before_ns == 0:
        return ("—", "")
    pct = ((after_ns - before_ns) / before_ns) * 100
    sign = "+" if pct > 0 else ""
    text = f"{sign}{pct:.2f}%"
    abs_pct = abs(pct)
    if abs_pct < THRESH_MINOR:
        icon = ""           # within noise band
    elif abs_pct < THRESH_MAJOR:
        icon = "⚠️" if pct > 0 else "✨"
    else:
        icon = "🚨" if pct > 0 else "🚀"
    return (text, icon)


# ---------------------------------------------------------------------------
# Comment rendering
# ---------------------------------------------------------------------------


def render_first_run(
    current: dict,
    version: str,
    sha: str,
    run_url: str,
) -> str:
    bench = current.get("benchmarks", {})
    lines: list[str] = []
    lines.append("## Benchmark results")
    lines.append("")
    lines.append("> First time this workflow recorded benchmarks for this PR — "
                 "there is no previous run to compare against yet. The numbers "
                 "below become the baseline; subsequent pushes will show a "
                 "side-by-side diff against them.")
    lines.append("")
    lines.append(f"**This run:** `{version}` · commit `{sha[:8]}` · "
                 f"[workflow run]({run_url})")
    lines.append("")

    if not bench:
        lines.append("_No benchmark data captured in this run._")
    else:
        lines.append("| Benchmark | Result | 95% CI |")
        lines.append("|---|---:|---:|")
        for name in sorted(bench):
            data = bench[name]
            mean = data.get("mean_ns", 0.0)
            lower = data.get("lower_ns", mean)
            upper = data.get("upper_ns", mean)
            lines.append(
                f"| `{name}` | {format_time(mean)} | {format_ci(lower, upper, mean)} |"
            )
        lines.append("")

    lines.append("---")
    lines.append(_legend())
    lines.append("")
    lines.append(encode_marker(current))
    return "\n".join(lines) + "\n"


def render_comparison(
    current: dict,
    previous: dict,
    version: str,
    sha: str,
    run_url: str,
) -> str:
    cur_bench = current.get("benchmarks", {})
    prev_bench = previous.get("benchmarks", {})
    prev_version = previous.get("version", "unknown")
    prev_commit = previous.get("commit", "unknown")

    common = sorted(set(cur_bench) & set(prev_bench))
    only_current = sorted(set(cur_bench) - set(prev_bench))
    only_previous = sorted(set(prev_bench) - set(cur_bench))

    n_regressions = 0
    n_improvements = 0

    lines: list[str] = []
    lines.append("## Benchmark results")
    lines.append("")
    lines.append(
        f"**Before:** `{prev_version}` · commit `{prev_commit[:8]}`  "
    )
    lines.append(
        f"**After:**  `{version}` · commit `{sha[:8]}` · "
        f"[workflow run]({run_url})"
    )
    lines.append("")

    if common:
        lines.append("| Benchmark | Before | After | Change |   |")
        lines.append("|---|---:|---:|---:|:---|")
        for name in common:
            prev_mean = prev_bench[name].get("mean_ns", 0.0)
            cur_mean = cur_bench[name].get("mean_ns", 0.0)
            delta_text, icon = format_delta(prev_mean, cur_mean)
            if icon in ("⚠️", "🚨"):
                n_regressions += 1
            elif icon in ("✨", "🚀"):
                n_improvements += 1
            lines.append(
                f"| `{name}` | {format_time(prev_mean)} | "
                f"{format_time(cur_mean)} | {delta_text} | {icon} |"
            )
        lines.append("")

    if only_current:
        lines.append(f"**New benchmarks ({len(only_current)})** — no previous data yet:")
        lines.append("")
        lines.append("| Benchmark | Result |")
        lines.append("|---|---:|")
        for name in only_current:
            mean = cur_bench[name].get("mean_ns", 0.0)
            lines.append(f"| `{name}` | {format_time(mean)} |")
        lines.append("")

    if only_previous:
        lines.append(f"**Removed benchmarks ({len(only_previous)})** — present "
                     "in the previous run but not in this one:")
        lines.append("")
        for name in only_previous:
            lines.append(f"- `{name}`")
        lines.append("")

    # Summary line
    lines.append(
        f"**Summary:** {len(common)} compared · "
        f"{n_improvements} faster · {n_regressions} slower · "
        f"{len(only_current)} new · {len(only_previous)} removed"
    )
    lines.append("")
    lines.append("---")
    lines.append(_legend())
    lines.append("")
    lines.append(encode_marker(current))
    return "\n".join(lines) + "\n"


def _legend() -> str:
    return (
        "<sub>Emoji legend: 🚀 large speedup · ✨ minor speedup · "
        "⚠️ minor regression · 🚨 large regression · "
        f"thresholds {THRESH_MINOR:.0f}% / {THRESH_MAJOR:.0f}%. "
        "Changes below the minor threshold are considered noise.</sub>"
    )


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--current", required=True, type=Path)
    parser.add_argument("--previous", type=Path,
                        help="Path to the previous sticky comment body (may be empty / missing)")
    parser.add_argument("--version", required=True)
    parser.add_argument("--sha", required=True)
    parser.add_argument("--run-url", required=True)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    if not args.current.is_file():
        sys.exit(f"error: {args.current} not found")

    current = json.loads(args.current.read_text())

    previous = None
    if args.previous and args.previous.is_file():
        previous = decode_marker(args.previous.read_text())

    if previous and previous.get("benchmarks"):
        body = render_comparison(current, previous,
                                 args.version, args.sha, args.run_url)
    else:
        body = render_first_run(current,
                                args.version, args.sha, args.run_url)

    args.output.write_text(body)
    print(f"wrote {args.output} ({len(body.splitlines())} lines)")


if __name__ == "__main__":
    main()
