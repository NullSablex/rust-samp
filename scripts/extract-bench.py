#!/usr/bin/env python3
"""Extracts a single benchmark history entry from criterion's `target/criterion/`.

Walks `target/criterion/**/new/estimates.json`, collects mean point estimate
and 95% confidence interval for each benchmark, and writes one JSON entry to
`bench-entry.json` in the project root.

Environment variables:
    VERSION  — release tag or "manual-<sha>" for workflow_dispatch runs
    GIT_SHA  — commit SHA at the time of the bench run
"""

from __future__ import annotations

import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path

CRITERION_DIR = Path("target/criterion")
OUTPUT = Path("bench-entry.json")


def collect_benchmarks() -> dict[str, dict[str, float]]:
    if not CRITERION_DIR.is_dir():
        # No criterion data — likely because `cargo bench` did not produce
        # any output (e.g. a workflow run that compiles benches without
        # executing them). Treat as an empty entry instead of hard-failing
        # so downstream steps still upload the (empty) artifact and the
        # workflow surface stays green when there is nothing to record.
        print(
            f"warning: {CRITERION_DIR} not found — emitting empty benchmark entry",
            file=sys.stderr,
        )
        return {}

    benchmarks: dict[str, dict[str, float]] = {}
    for est in CRITERION_DIR.rglob("new/estimates.json"):
        rel = est.relative_to(CRITERION_DIR)
        # rel = <group>/<id>/.../new/estimates.json  → drop trailing 'new/estimates.json'
        bench_id = "/".join(rel.parts[:-2])
        with est.open() as fh:
            data = json.load(fh)
        mean = data.get("mean")
        if not mean:
            continue
        ci = mean.get("confidence_interval", {})
        benchmarks[bench_id] = {
            "mean_ns": mean["point_estimate"],
            "lower_ns": ci.get("lower_bound", mean["point_estimate"]),
            "upper_ns": ci.get("upper_bound", mean["point_estimate"]),
        }
    return benchmarks


def main() -> None:
    version = os.environ.get("VERSION", "unknown")
    commit = os.environ.get("GIT_SHA", "unknown")
    entry = {
        "version": version,
        "commit": commit,
        "timestamp": datetime.now(timezone.utc).isoformat().replace("+00:00", "Z"),
        "benchmarks": collect_benchmarks(),
    }
    OUTPUT.write_text(json.dumps(entry, indent=2) + "\n")
    print(f"wrote {OUTPUT} with {len(entry['benchmarks'])} benchmarks")


if __name__ == "__main__":
    main()
