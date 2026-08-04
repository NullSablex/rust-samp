#!/usr/bin/env bash
# Run the SDK benchmarks locally.
#
# Benchmarks are intentionally NOT part of CI (no GitHub Actions run them on
# push/PR/release) — they are noisy on shared runners and provide little signal
# there. Run them on your own machine when you want to measure a change:
#
#   scripts/bench.sh                 # default: criterion run on i686
#   scripts/bench.sh --save-baseline main   # save a baseline to compare against
#   scripts/bench.sh --baseline main        # compare against a saved baseline
#
# Any extra arguments are forwarded to `cargo bench` (and thus to criterion).
#
# The benches target i686 to match the arch SA-MP/open.mp actually run on.
set -euo pipefail

TARGET="${BENCH_TARGET:-i686-unknown-linux-gnu}"

exec cargo bench -p rust-samp-sdk --target "$TARGET" "$@"
