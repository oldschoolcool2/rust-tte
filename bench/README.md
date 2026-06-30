# `bench/` — Phase-5 benchmark + reproducibility harness

Reproduces the runtime/memory curves ([`report/benchmark.md`](../report/benchmark.md))
and the Tier-2 whole-pipeline golden ([`report/golden.md`](../report/golden.md)).
Every input here is **deterministically generated** (seeded); nothing in this
directory reads or writes the immutable `fixtures/`.

| file | purpose |
|---|---|
| `gen.R` | fast, vectorized, **seeded** input generator (engine input schema) |
| `prep.R` | times upstream `TrialEmulation::data_preparation` (the baseline) |
| `run_bench.sh` | R-vs-Rust runtime + peak-RSS sweep → `report/benchmark_data.md` |
| `runner/` | standalone Rust CLI (`expand_parquet`) timed via `/usr/bin/time -v` |
| `golden_roundtrip.R`, `run_golden.sh` | Tier-2 Rust-expand → R-estimate golden |

## Run

```sh
make bench         # criterion runtime sweep of the engine (in-memory transform)
make bench-smoke   # fast smoke (input rows <= 1e5, criterion --quick) — CI
make curves        # R-vs-Rust runtime + peak-RSS table (heavy, needs R)
make golden        # Tier-2 whole-pipeline golden (needs R)
```

The criterion harness itself lives in
[`crates/tte-expand/benches/`](../crates/tte-expand/benches/)
(`expand.rs` = timing, `certificate.rs` = the reproducibility certificate).

## Notes

- **`runner/` is its own Cargo workspace** (own `Cargo.lock`), so the root
  `tte-expand` workspace ignores it; `cargo build/test/clippy/deny` at the repo
  root never touch it. It is compiled only by `run_bench.sh` / `run_golden.sh`.
  It depends on the engine by path, so it always times the exact verified core.
- **`curves` / `golden` are heavy and manual** — they need R + `TrialEmulation` +
  GNU `/usr/bin/time`, and `curves` can drive R into its OOM regime (guarded by
  `ulimit -v`). CI runs only `make verify` + `make bench-smoke`.
- **Sweep override:** `SIZES="2000 20000 200000 2000000" MEMCAP_KB=8000000 bash bench/run_bench.sh`.
