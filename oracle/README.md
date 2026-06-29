# oracle/ — the R Oracle & fixture generation

**Everything here is read-only to the engine (and to the build agent).** These
scripts turn the `TrialEmulation` R package into the immutable **Oracle**: they
run the upstream expansion on seed / simulated / edge-case cohorts and dump
`input_*.parquet` + `expected_*.parquet` into `../fixtures/`, recording a sha256
manifest. The Rust engine's only job is to match those fixtures.

> **Ground-truth rule:** never "fix" the Oracle to make Rust pass. If the Oracle
> disagrees with an expectation, that is a *finding to investigate*, not a bug to
> paper over.

## ⚠️ VERIFY FIRST (before trusting any output)

`TrialEmulation` is under active development; expanded column names can shift
between releases. **Freeze `STRUCTURAL_COLS` from `names(prep$data)` on your
installed version** before generating fixtures:

```r
library(TrialEmulation)
data(data_censored)
str(data_censored)                 # confirm exact columns + types
prep <- data_preparation(
  data_censored, id = "id", period = "period", treatment = "treatment",
  outcome = "outcome", eligible = "eligible", estimand_type = "ITT",
  outcome_cov = ~1, use_censor_weights = FALSE,
  data_dir = tempfile() |> (\(d){dir.create(d); d})(), separate_files = FALSE, quiet = TRUE
)
names(prep$data)                   # <-- FREEZE STRUCTURAL_COLS from THIS
```

See `../docs/001-initial-ideations/004-prework-fixtures.md` for the full
rationale, the edge-case alignment matrix, and the three-tier validation map.

## Files

| File | Purpose |
|---|---|
| `00_setup.R` | Environment, pinned constants (`STRUCTURAL_COLS`, `INPUT_COLS`), provenance. |
| `10_simulate.R` | Self-contained longitudinal DGP (time-varying confounding). |
| `20_scenarios.R` | Named registry: common → ultra-rare → stress cohorts. |
| `30_edge_cases.R` | Hand-authored immortal-time landmine cohorts (needs epi sign-off). |
| `40_dump_fixtures.R` | Runs the Oracle (ITT + PP), writes Parquet + manifest entries. |
| `50_golden_pipeline.R` | Tier-2 whole-pipeline goldens (tolerance-based). |
| `60_harvest_upstream.R` | Convert upstream `tests/testthat` snapshots to Parquet. |
| `run_all.R` | Orchestrator: regenerate the full fixture set deterministically. |

## Reproducibility

- Pin R + all package versions with `renv` (`renv::init()`, then `renv::snapshot()`
  → `renv.lock`, committed here).
- Pin a specific upstream commit (submodule or `remotes::install_github(ref=...)`).
- CI regenerates fixtures from the pinned Oracle and fails on any sha256 drift, so
  the contract can never silently change underneath the engine.

## Run

```sh
# from the repository root, with the pinned R environment active:
Rscript oracle/run_all.R
```
