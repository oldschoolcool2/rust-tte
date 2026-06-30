#!/usr/bin/env bash
# bench/run_golden.sh — build the Rust runner, then run the Tier-2 whole-pipeline
# golden (bench/golden_roundtrip.R). Needs R + TrialEmulation. Run via `make golden`.
set -euo pipefail
export PATH="$HOME/.cargo/bin:$PATH"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
echo ">> building Rust runner (release)..." >&2
cargo build --release --manifest-path "$ROOT/bench/runner/Cargo.toml" >&2
RUNNER="$ROOT/bench/runner/target/release/runner"
Rscript "$ROOT/bench/golden_roundtrip.R" "$RUNNER" "$ROOT"
