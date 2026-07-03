# tte-expand — Pre-Work: Fixture Generation Code

**Companion to `tte-expand-project-plan.md`. This is Phase 0 made concrete:** the R code that turns the `TrialEmulation` package into your Oracle and produces the three tiers of validation material the agent will build against. Everything here lives in `oracle/` and is **read-only to the agent**.

Three production lines feed three validation tiers:

1. **Simulated cohorts** (common → ultra-rare → edge structures) → poured through the Oracle → exact-match expansion fixtures.
2. **Hand-authored edge cases** (immortal-time landmines) → the part needing your epi sign-off.
3. **Harvested upstream tests + whole-pipeline goldens** → integration validation after the extendr binding exists.

---

## VERIFY FIRST (before trusting any output below)

These were inferred from the package docs; confirm against the installed version, because they shift between releases:

```r
library(TrialEmulation)
data(data_censored)
str(data_censored)                 # confirm exact columns + types
prep <- data_preparation(
  data_censored, id="id", period="period", treatment="treatment",
  outcome="outcome", eligible="eligible", estimand_type="ITT",
  outcome_cov=~1, use_censor_weights=FALSE,
  data_dir=tempfile() |> (\(d){dir.create(d); d})(), separate_files=FALSE, quiet=TRUE
)
names(prep$data)                   # <-- FREEZE your structural column set from THIS
# Look specifically for: id, trial_period, followup_time,
#   assigned_treatment, treatment, outcome, and any PP censoring/expand flag.
```

Whatever `names(prep$data)` shows is the truth; the `STRUCTURAL_COLS` constant below must match it. If a column you expect is missing or renamed, fix the constant, not the Oracle.

> The newer S4 path (`trial_sequence("ITT") |> set_data() |> set_expansion_options(save_to_datatable()) |> expand_trials()`) separates expansion from weighting more cleanly, which is *nicer* for pure-structure fixtures — but the accessor for the expanded table varies by version. Use legacy `data_preparation()$data` until you've confirmed the S4 accessor, then optionally switch.

---

## `oracle/00_setup.R`

```r
# oracle/00_setup.R — environment + pinned constants
# Run once: renv::init(); then after install: renv::snapshot() to pin versions.

suppressPackageStartupMessages({
  library(TrialEmulation)
  library(arrow)      # Parquet I/O (preserves dtypes; never use CSV for fixtures)
  library(digest)     # sha256 of fixtures for the manifest
  library(jsonlite)   # manifest + golden JSON
  library(data.table)
})

# The deterministic columns the Rust engine must reproduce bit-for-bit.
# FREEZE this from names(prep$data) on your installed version (see VERIFY FIRST).
STRUCTURAL_COLS <- c(
  "id", "trial_period", "followup_time",
  "assigned_treatment", "treatment", "outcome"
  # add a PP censoring/expand-flag column here IF your version emits one
)

# Input schema every cohort (simulated, edge, harvested) must conform to.
INPUT_COLS <- c("id", "period", "eligible", "treatment", "x1", "x2", "outcome")

OUT_ROOT <- "fixtures"  # written relative to repo root

# Provenance stamped into every manifest entry so fixtures are traceable.
ORACLE_PROVENANCE <- list(
  package         = "TrialEmulation",
  package_version = as.character(packageVersion("TrialEmulation")),
  r_version       = R.version.string,
  generated_utc   = format(as.POSIXct(Sys.time(), tz = "UTC"), "%Y-%m-%dT%H:%M:%SZ")
)
```

---

## `oracle/10_simulate.R` — the data-generating process

A self-contained, fully-parameterised longitudinal DGP. It emits the exact input schema and exposes knobs for every structural axis you care about. (You can later swap in the published simulator from `juliettelimozin/Multiple-trial-emulation-IPTW-MSM-CIs` as a richer generator — but this one is correct, runnable today, and has no external API to guess. Pour either through the same Oracle dump.)

```r
# oracle/10_simulate.R — longitudinal cohort simulator for sequential TTE
source("oracle/00_setup.R")

# Returns one person-period long data.frame in INPUT_COLS schema.
# Epidemiology baked in:
#  - x1 is a time-varying confounder (AR(1)) affecting BOTH initiation and outcome
#    => genuine time-varying confounding, the thing IPCW exists to handle.
#  - eligibility = treatment-naive at start of period (recurrent until initiation)
#    => one patient legitimately seeds MULTIPLE trial_periods (the core behaviour).
#  - switch_prob > 0 lets treated patients deviate => exercises PP artificial censoring.
#  - a person's rows stop at first of outcome / censoring / max_period.
simulate_cohort <- function(n, max_period, params, seed) {
  set.seed(seed)
  p <- modifyList(list(
    L_ar = 0.8, L_sd = 0.7, x2_int = -0.2,     # confounder dynamics
    init_int = -2.0, conf_AL = 0.8,            # treatment initiation (hazard, confounded by L)
    out_int  = -3.0, beta_A = -0.5, conf_YL = 0.7,  # outcome hazard
    cens_prob = 0.02, switch_prob = 0.0        # censoring; switch_prob=0 => absorbing treatment
  ), params)

  out_list <- vector("list", n)
  for (i in seq_len(n)) {
    L <- rnorm(1)
    A_prev <- 0L
    rows_i <- vector("list", max_period + 1L)
    k <- 0L
    for (t in 0:max_period) {
      L  <- p$L_ar * L + rnorm(1, 0, p$L_sd)
      x2 <- rbinom(1, 1, plogis(p$x2_int + 0.5 * L))
      eligible <- as.integer(A_prev == 0L)            # naive => eligible
      if (A_prev == 1L) {
        A <- if (runif(1) < p$switch_prob) 0L else 1L # may deviate (PP stress)
      } else {
        A <- rbinom(1, 1, plogis(p$init_int + p$conf_AL * L))
      }
      Y <- rbinom(1, 1, plogis(p$out_int + p$beta_A * A + p$conf_YL * L))
      C <- rbinom(1, 1, p$cens_prob)
      k <- k + 1L
      rows_i[[k]] <- data.frame(
        id = i, period = t, eligible = eligible,
        treatment = A, x1 = L, x2 = x2, outcome = Y
      )
      A_prev <- A
      if (Y == 1L || C == 1L) break
    }
    out_list[[i]] <- do.call(rbind, rows_i[seq_len(k)])
  }
  df <- do.call(rbind, out_list)
  rownames(df) <- NULL
  df[, INPUT_COLS]
}

# DGP self-check: assert the simulator produces well-formed input before it ever
# reaches the Oracle. Catches a broken generator early (cheaper than a failed dump).
validate_input <- function(df) {
  stopifnot(
    all(INPUT_COLS %in% names(df)),
    all(df$eligible %in% c(0L, 1L)),
    all(df$treatment %in% c(0L, 1L)),
    all(df$outcome %in% c(0L, 1L)),
    # per id, periods are contiguous from their first value
    df[, all(diff(sort(unique(period))) == 1L), by = id][, all(V1)]
  )
  invisible(df)
}
```

---

## `oracle/20_scenarios.R` — common → ultra-rare → stress registry

A named registry from "scenario" → simulator params, spanning the structural spectrum. Each becomes a batch of exact-match fixtures. Tune the `prevalence`/rarity dials to align with the event rates you actually see in RWD (e.g. an oncology safety signal vs. a common cardiometabolic outcome).

```r
# oracle/20_scenarios.R
source("oracle/10_simulate.R")

SCENARIOS <- list(
  common = list(
    desc = "Workhorse: moderate initiation, common events, light censoring.",
    n = 300, max_period = 12, seed = 101,
    params = list(init_int = -1.5, out_int = -2.5, cens_prob = 0.03)
  ),
  rare_event = list(
    desc = "Common exposure, rare outcome (typical pharmacoepi safety study).",
    n = 800, max_period = 18, seed = 102,
    params = list(init_int = -1.5, out_int = -4.5, cens_prob = 0.03)
  ),
  ultra_rare_event = list(
    desc = "Ultra-rare outcome; many trials, almost no events (numerics stress).",
    n = 2000, max_period = 18, seed = 103,
    params = list(init_int = -1.5, out_int = -6.5, cens_prob = 0.03)
  ),
  rare_initiation = list(
    desc = "Treatment seldom started => long recurrent-eligibility runs per id.",
    n = 600, max_period = 18, seed = 104,
    params = list(init_int = -3.5, out_int = -3.0, cens_prob = 0.03)
  ),
  high_switching = list(
    desc = "Frequent deviation => maximal ITT-vs-PP divergence.",
    n = 400, max_period = 15, seed = 105,
    params = list(init_int = -1.0, switch_prob = 0.25, out_int = -3.0)
  ),
  heavy_censoring = list(
    desc = "High dropout => short, ragged follow-up; many truncated trials.",
    n = 500, max_period = 15, seed = 106,
    params = list(cens_prob = 0.20, out_int = -3.0)
  ),
  short_followup = list(
    desc = "Few periods => many single-/few-row trials.",
    n = 400, max_period = 3, seed = 107,
    params = list(init_int = -1.0, out_int = -2.5)
  ),
  strong_confounding = list(
    desc = "Strong L->A and L->Y => stresses downstream weighting (Tier 2/3).",
    n = 600, max_period = 15, seed = 108,
    params = list(conf_AL = 1.6, conf_YL = 1.6, out_int = -3.0)
  ),
  large_scale = list(
    desc = "Volume/memory shakeout (use for benchmarks, not unit fixtures).",
    n = 20000, max_period = 24, seed = 109,
    params = list(init_int = -1.5, out_int = -3.5)
  )
)

build_scenario <- function(name) {
  s <- SCENARIOS[[name]]
  stopifnot(!is.null(s))
  validate_input(simulate_cohort(s$n, s$max_period, s$params, s$seed))
}
```

---

## `oracle/30_edge_cases.R` — the immortal-time landmine catalog

Tiny, deterministic, hand-typed cohorts that probe the exact places sequential expansion goes wrong. **These require epidemiological review** — the comments state what each *should* exercise, but the Oracle output is canonical: if the Oracle disagrees with an expectation comment, that's a finding to investigate (possibly the eligibility model here is wrong, possibly a real subtlety), not a bug to paper over.

```r
# oracle/30_edge_cases.R
source("oracle/00_setup.R")

mk <- function(...) {
  df <- data.frame(..., stringsAsFactors = FALSE)
  for (c in c("eligible","treatment","outcome","x2")) df[[c]] <- as.integer(df[[c]])
  if (is.null(df$x1)) df$x1 <- 0
  df[, INPUT_COLS]
}

EDGE_CASES <- list(

  # E01 — minimal: one eligible patient, one period. Expect: trial_period=0,
  # followup_time=0, single row. Floor case for the whole engine.
  E01_single = mk(id=1, period=0, eligible=1, treatment=0, x1=0, x2=0, outcome=0),

  # E02 — CANONICAL vignette ID=4: eligible t=0,1,2; initiates at t=2; followed to t=9.
  # Expect (ITT): trial 0 assigned=0 fu 0..9; trial 1 assigned=0 fu 0..8;
  #               trial 2 assigned=1 fu 0..7. This is the published reference behaviour.
  E02_id4_canonical = mk(
    id=4, period=0:9,
    eligible  =c(1,1,1,0,0,0,0,0,0,0),
    treatment =c(0,0,1,1,1,1,1,1,1,1),
    x1=0, x2=0, outcome=0
  ),

  # E03 — event ON a trial baseline visit (followup_time=0). Probes whether a
  # baseline-visit outcome is retained vs dropped. Classic off-by-one risk.
  E03_event_at_baseline = mk(id=1, period=0, eligible=1, treatment=0, x1=0, x2=0, outcome=1),

  # E05 — never initiates: eligible every period, control-only max fan-out.
  # Expect a trial seeded at EVERY period, all assigned=0.
  E05_never_treats = mk(
    id=1, period=0:4, eligible=1, treatment=0, x1=0, x2=0, outcome=0
  ),

  # E07 — eligibility ONLY in the final observed period => single-row trial at the edge.
  E07_last_period_only = mk(
    id=1, period=0:3,
    eligible  =c(0,0,0,1),
    treatment =c(1,1,1,0),  # was treated, then a (contrived) naive final period
    x1=0, x2=0, outcome=0
  )

  # ------------------------------------------------------------------
  # Pending epidemiological review and literature alignment: specify these precisely.
  # Each is a known place sequential expansion / time-zero alignment fails.
  #
  # E04_reentry        : eligible -> ineligible -> eligible again.
  #                      Question: does a re-entered eligibility correctly seed a
  #                      NEW trial_period, with assigned_treatment from the re-entry
  #                      period? (immortal-time + re-entry interaction)
  #                      Anchor: Fu/Hernan BMJ 2026 e084909 (time-zero alignment).
  #
  # E06_switch_then_back (PP): A=1 then 0 then 1. Where exactly does PP artificial
  #                      censoring fire, and does it fire on the FIRST deviation only?
  #                      Anchor: Danaei 2013; package expand_until_switch semantics.
  #
  # E08_ties           : simultaneous event/censor on the same visit. Tie-break order?
  #
  # E09_max_fanout     : 1 patient eligible & untreated for many periods (e.g. 0..30)
  #                      => stress row-count invariant + memory in miniature.
  # ------------------------------------------------------------------
)
```

**Alignment matrix (to be completed during epidemiological review; drop into the doc/PR as the rationale):**

| ID | Probes | Bias / mechanism | Literature anchor | Status |
|----|--------|------------------|-------------------|--------|
| E01 | floor case | — | — | ready |
| E02 | multi-trial seeding | recurrent eligibility | vignette ID=4 / Danaei 2013 | ready |
| E03 | baseline-visit event | off-by-one at time zero | Fu/Hernán 2026 | ready |
| E04 | eligibility re-entry | immortal time + re-entry | Fu/Hernán 2026 | under review |
| E05 | control-only fan-out | — | — | ready |
| E06 | first-deviation censoring | PP artificial censoring | Danaei 2013 | under review |
| E07 | edge single-row trial | last-period eligibility | — | ready |
| E08 | tie handling | event/censor tie-break | — | under review |
| E09 | max fan-out | row-count invariant | — | under review |

---

## `oracle/40_dump_fixtures.R` — the Oracle harness

Takes any cohort, runs the Oracle for ITT and PP, writes `input_*` + `expected_*` Parquet (structural columns only), and records sha256 + provenance into the manifest. This is the immutable contract the agent matches.

```r
# oracle/40_dump_fixtures.R
source("oracle/00_setup.R")

# Run the Oracle once and return ONLY the deterministic structural columns.
oracle_expand <- function(cohort, estimand) {
  td <- tempfile("te_"); dir.create(td)
  on.exit(unlink(td, recursive = TRUE), add = TRUE)
  prep <- data_preparation(
    data = cohort, id = "id", period = "period", treatment = "treatment",
    outcome = "outcome", eligible = "eligible",
    estimand_type = estimand, outcome_cov = ~1,
    use_censor_weights = FALSE,                 # keep expansion free of glm
    data_dir = td, separate_files = FALSE, quiet = TRUE
  )
  expanded <- as.data.frame(prep$data)
  keep <- intersect(STRUCTURAL_COLS, names(expanded))
  if (!setequal(keep, STRUCTURAL_COLS))
    warning("STRUCTURAL_COLS mismatch for ", estimand, ": have {",
            paste(names(expanded), collapse=", "), "}")
  # deterministic ordering so byte-comparison is stable
  ord <- do.call(order, expanded[intersect(c("id","trial_period","followup_time"), keep)])
  expanded[ord, keep, drop = FALSE]
}

dump_fixture <- function(cohort, name, subdir, estimands = c("ITT","PP")) {
  out_dir <- file.path(OUT_ROOT, subdir); dir.create(out_dir, recursive = TRUE, showWarnings = FALSE)
  in_path <- file.path(out_dir, sprintf("input_%s.parquet", name))
  arrow::write_parquet(cohort, in_path)
  entries <- list(list(
    role = "input", path = in_path, sha256 = digest(file = in_path, algo = "sha256"),
    n_rows = nrow(cohort)
  ))
  for (est in estimands) {
    expected <- oracle_expand(cohort, est)
    ep <- file.path(out_dir, sprintf("expected_%s_%s.parquet", name, tolower(est)))
    arrow::write_parquet(expected, ep)
    entries[[length(entries)+1]] <- list(
      role = paste0("expected_", tolower(est)), path = ep,
      sha256 = digest(file = ep, algo = "sha256"), n_rows = nrow(expected)
    )
  }
  entries
}

write_manifest <- function(all_entries, path = file.path(OUT_ROOT, "MANIFEST.json")) {
  jsonlite::write_json(
    list(provenance = ORACLE_PROVENANCE, fixtures = all_entries),
    path, pretty = TRUE, auto_unbox = TRUE
  )
  message("Manifest -> ", path)
}
```

---

## `oracle/50_golden_pipeline.R` — Tier-2 whole-pipeline goldens

Run the *entire* package pipeline (expansion + weights + MSM + robust SE) on the shipped data and freeze the coefficient table. After your extendr binding exists (Phase 4), `Rust-expansion + R-estimation` must reproduce these **within documented tolerance** — that's the end-to-end "no gluing errors" check. This is *not* bit-exact; it's the tier where `parglm`/`sandwich` live.

```r
# oracle/50_golden_pipeline.R
source("oracle/00_setup.R")

golden_pipeline <- function(data, name, out_dir = file.path(OUT_ROOT, "golden")) {
  dir.create(out_dir, recursive = TRUE, showWarnings = FALSE)
  res <- initiators(
    data = data, id = "id", period = "period", treatment = "treatment",
    outcome = "outcome", eligible = "eligible",
    estimand_type = "ITT", model_var = "assigned_treatment",
    outcome_cov = ~1, use_censor_weights = FALSE, quiet = TRUE
  )
  # VERIFY accessor on your version: summary(res) prints a robust coef table with
  # columns names/estimate/robust_se/2.5%/97.5%/z/p_value. Grab the data behind it.
  robust_tab <- tryCatch(res$robust$summary, error = function(e) NULL)
  if (is.null(robust_tab)) robust_tab <- summary(res)$coefficients  # fallback
  jsonlite::write_json(
    list(provenance = ORACLE_PROVENANCE, dataset = name,
         tolerance = list(log_or = 1e-4, robust_se = 1e-3),  # documented contract
         coefficients = robust_tab),
    file.path(out_dir, sprintf("golden_%s_itt.json", name)),
    pretty = TRUE, digits = 12, auto_unbox = TRUE
  )
  message("Golden -> ", name)
}
```

---

## `oracle/60_harvest_upstream.R` — reuse the maintainers' own expectations

The cheapest authoritative fixtures already exist in the upstream `tests/testthat/`. Harvest them rather than reinvent. Their snapshots are R `.rds`; convert any expansion-stage expectation to Parquet so the Rust harness can read it.

```r
# oracle/60_harvest_upstream.R
source("oracle/00_setup.R")

# 1. Vendor the repo (pin a commit) as a submodule under oracle/TrialEmulation, OR:
#    remotes::install_github("Causal-LDA/TrialEmulation", ref = "<COMMIT_SHA>")
#
# 2. Grep their tests for expansion-stage assertions (skip glm/weight/coef tests):
#      tests/testthat/  ->  look for expand / data_preparation / trial_period /
#      followup_time / expand_until_switch; ignore *weight*, *msm*, *robust*.
#
# 3. Convert any .rds expected-output snapshot to a structural Parquet fixture:
rds_to_fixture <- function(rds_path, name, subdir = "harvested") {
  obj <- readRDS(rds_path)
  df  <- if (is.data.frame(obj)) obj else as.data.frame(obj$data %||% obj)
  keep <- intersect(STRUCTURAL_COLS, names(df))
  out_dir <- file.path(OUT_ROOT, subdir); dir.create(out_dir, recursive = TRUE, showWarnings = FALSE)
  p <- file.path(out_dir, sprintf("expected_%s.parquet", name))
  arrow::write_parquet(df[, keep, drop = FALSE], p)
  message("Harvested ", rds_path, " -> ", p)
  p
}
`%||%` <- function(a, b) if (is.null(a)) b else a
```

> Caveat from doc #1: the upstream suite is a *regression* suite over standard example data — it locks in known behaviour rather than probing edge cases. Reuse it for authoritative baseline coverage, then rely on §20 (scenarios) and §30 (edge cases) for the difficult trajectories it doesn't exercise.

---

## `oracle/run_all.R` — orchestrator

```r
# oracle/run_all.R — regenerate the full fixture set deterministically.
source("oracle/20_scenarios.R")
source("oracle/30_edge_cases.R")
source("oracle/40_dump_fixtures.R")
source("oracle/50_golden_pipeline.R")

entries <- list()

# Tier 1a — simulated scenarios (exact-match)
for (nm in names(SCENARIOS)) {
  if (nm == "large_scale") next                      # benchmark only, not a unit fixture
  cohort <- build_scenario(nm)
  entries <- c(entries, dump_fixture(cohort, nm, subdir = "scenarios"))
}

# Tier 1b — hand-authored edge cases (exact-match)
for (nm in names(EDGE_CASES)) {
  entries <- c(entries, dump_fixture(EDGE_CASES[[nm]], nm, subdir = "edge"))
}

# Tier 2 — whole-pipeline goldens (tolerance-based)
data(data_censored); golden_pipeline(data_censored, "data_censored")
data(trial_example); golden_pipeline(trial_example, "trial_example")

write_manifest(entries)
cat("\nDone. Fixtures under '", OUT_ROOT, "'. CI must regenerate and diff sha256.\n", sep = "")
```

---

## The three-tier validation map

| Tier | Question | Source (this doc) | Comparison | Where it runs |
|------|----------|-------------------|-----------|---------------|
| 1 — Fidelity | Does Rust expansion == R expansion? | §20 scenarios + §30 edges + §60 harvested | **bit-exact** on `STRUCTURAL_COLS` | every `cargo test` (Phases 1–3) |
| 2 — Integration | Does Rust-expand + R-estimate hit a known end-to-end number? | §50 goldens | **tolerance** (log-OR 1e-4, SE 1e-3) | after extendr binding (Phase 4) |
| 3 — Science | Is the *method* unbiased? | §20 with known `beta_A` | recover true effect ± MC error | optional, simulation study |

Tier 1 is the contract the agent satisfies. Tier 2 catches gluing errors the bit-exact tests can't see. Tier 3 is a research-grade nicety, not a build gate.

---

## Decisions requiring domain (epidemiological) sign-off

1. **Freeze `STRUCTURAL_COLS`** from `names(prep$data)` on your installed version — including whether PP emits a censoring/expand flag to add.
2. **Finish the edge catalog** (E04, E06, E08, E09) — these are the immortal-time landmines, and which trajectories are *dangerous* is epidemiology, not code. The alignment matrix is where you record the rationale (and it becomes PR/paper material).
3. **Set the rarity dials** in §20 to match the RWD event rates you actually care about (oncology safety vs. common cardiometabolic), so "ultra-rare" means something real.
4. **Confirm the Tier-2 tolerances** in §50 are scientifically acceptable to you — that number is a claim you're standing behind.

---

## How this plugs into doc #1

This *is* the first-week checklist of `tte-expand-project-plan.md`, now executable:

- `oracle/run_all.R` → produces `fixtures/` (the immutable contract) + `MANIFEST.json`.
- The agent never sees this code — it only reads `fixtures/*.parquet` + `SPEC.md`.
- CI runs `run_all.R` against the pinned Oracle and fails on any sha256 drift (ADR-1, differential CI).
- `STRUCTURAL_COLS` here = the "match EXACTLY" column list in the Phase 1 prompt (§9b of doc #1).

Next buildable artifact: `SPEC.md` (the R-free behavioural spec) + the empty `cargo test` harness that loads these Parquet fixtures and asserts against `unimplemented!()`. That's the bridge to the first green loop.
```
