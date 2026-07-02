# Phase 11 — the `te_datastore` companion backend (summary)

> **Status: complete.** The `tters` Rust + Polars engine now plugs into upstream
> `TrialEmulation` through its own `te_datastore` extension API: a user's
> `trial_sequence()` pipeline runs the expensive **expansion in Rust** while
> estimation (sampling + the marginal structural model) stays in **R**, consuming
> the Rust output — **structural columns bit-identical**, weights and `fit_msm`
> coefficients within tolerance (rel **1e-12 / 1e-6**) — versus the default path.
> Opt-in, `Suggests`-level,
> with graceful fallback. **No Rust changed** — this is R integration glue over
> the Phase-8/9 in-memory binding.

This is **Track C** of the remaining work (the maintainer-green-lit companion
backend, `Causal-LDA/TrialEmulation#243`). It embodies the load-bearing split the
project is built on: **Rust owns deterministic data transformation; R owns
statistical estimation.**

---

## What was implemented

All changes are R-side, under `bindings/tters/` (one new R file + docs + tests +
a `DESCRIPTION` `Suggests`). The verified Rust core and the existing `tters`
contract suite are untouched; both lockfiles are unchanged.

### D1 — a `te_datastore`-conformant `tters` backend
`R/te-datastore-tters.R` defines, **conditionally at load time** (the parent
`te_datastore` class only exists when `TrialEmulation` is installed, so the
subclass cannot be declared at collation time for a `Suggests` dependency):

- **`te_datastore_tters`** — `setClass(contains = "te_datastore", slots = c(data = "data.table"))`, inheriting the `@N` integer slot.
- **`save_to_tters()`** — the `save_to_<x>()` constructor convention; returns `new("te_datastore_tters", data = data.table(), N = 0L)`, no saving.
- **`save_expanded_data`** — appends the chunk and refreshes `@N` (mirrors `te_datastore_datatable`; honours the multi-call/id-chunk contract even though the companion saves in one call).
- **`read_expanded_data(object, period, subset_condition)`** — `trial_period %in% period` filter + `str2lang()`/`eval()` subset semantics, identical to the data.table backend; base-R `period` validation (no new `checkmate` dep).
- **`show`** — class banner + `@N` + a head of the data.

`sample_expanded_data` is **deliberately not overridden**: the base method
(dispatching on the `te_datastore` parent) snapshots/restores the RNG,
`set.seed(seed)`, calls our `read_expanded_data()`, then splits by
`trial_period × followup_time` and `do_sampling`. Inheriting it makes seeded
sampling **bit-identical** to the data.table store. `load_expanded_data` /
`sample_controls` are **not** implemented (they dispatch on `trial_sequence`).

### D2 — a Rust-fast expansion entry that yields the datastore
**`expand_trials_tters(object, fallback = TRUE, quiet = FALSE)`** — a drop-in for
`TrialEmulation::expand_trials()`. It reads everything from the configured
`trial_sequence`, runs the expansion via the `tters` `*_df` path (**not** R's
`expand()`), maps the result to the **exact** keeplist frame, and threads it into
the registered datastore via `save_expanded_data`. The mapping
(`.tters_expanded_frame`, the make-or-break of the phase):

1. `estimand <- if (object@expansion@censor_at_switch) "PP" else "ITT"`.
2. `keeplist <- unique(c("id","trial_period","followup_time","outcome","weight","treatment", adjustment_vars, treatment_var))` — exactly as `expand_trials_trial_seq()`.
3. Clamp the period window to the observed eligible range (mirrors the upstream pre-loop clamp).
4. Read the **R-computed** per-period weight `wt` verbatim (defaulting to 1 when no weights were calculated) and pass it as the `tters` factor table → `expand_trial_weighted_df()` runs the expansion **and** the cumulative-weight accumulation in Rust.
5. **Re-sort** the Rust output (native order `(id, trial_period, followup_time)`) to `TrialEmulation`'s stored generation/`index` order `(id, period_new = trial_period + followup_time, trial_period)`.
6. **Join the baseline adjustment covariates** at `(id, period == trial_period)` — their value at the trial's start, broadcast across follow-up (a keyed lookup preserving row order).
7. Select `keeplist` (exact columns, order, dtypes).

### D4 — opt-in + graceful fallback (`Suggests`-level)
`DESCRIPTION` gains `Suggests: TrialEmulation, data.table` (never `Imports`). All
upstream-coupled code is gated behind `requireNamespace("TrialEmulation")`. The
backend is registered idempotently in `.onLoad` (and on first use). Any failure
of the Rust path — `TrialEmulation` absent, or the not-yet-supported AT estimand —
**falls back** to `TrialEmulation::expand_trials()` with a message (`fallback =
FALSE` forces the Rust path and surfaces the error). Plain `tters` (no
`TrialEmulation`) is wholly unaffected — the companion is a no-op for anyone who
does not opt in.

### D5 — docs + maintainer-facing artifact
A worked example in `bindings/tters/README.md` (`library(TrialEmulation); …;
expand_trials_tters(); fit_msm()`), this summary, and the one-line "Extending"
pointer to offer the maintainers (below) — **no PR is opened against the upstream
repo**.

---

## VERIFY-FIRST findings (resolved empirically before building, signed off)

Everything below was proven on `TrialEmulation`'s bundled `data_censored` cohort
(725 person-periods, 89 ids) against `TrialEmulation` **v0.0.4.11** / R 4.3.3.

| # | Question | Finding |
|---|---|---|
| **(a)** | **The SEAM** | Confirmed against `R/expand_trials.R` + `R/data_extension.R`: `expand_trials_trial_seq()` runs R's `expand()` **first**, then `save_expanded_data(datastore, switch_data)`. `te_datastore` is a **storage** seam, so a mere storage backend gives no speedup. ⇒ the companion runs the **expansion in Rust** and presents it **through** the interface. |
| **(b)** | **The DATA CONTRACT** | keeplist + dtypes nailed (`int,int,int,dbl,dbl,dbl,…passthrough…`); `treatment_var = "assigned_treatment"` present for ITT/PP. The mapping above reproduces the stored frame **structurally bit-exact** with `weight` to **6.7e-16 / 2.2e-16** (machine ε, far inside the enforced **1e-12** weight-application tolerance) across ITT-unweighted, ITT-weighted (IPCW), and PP-weighted (switch+censor). The make-or-break was the **row-order re-sort** (`id, period_new, trial_period`) — tters' native order differs from the stored `index` order. |
| **(c)** | **DOWNSTREAM PARITY** | Inheriting the base `sample_expanded_data` (→ our `read_expanded_data`) gives RNG-identical sampling. With seeded `load_expanded_data(seed = 1234, p_control = 0.5)`: sampled `outcome_data` **identical**, no-sample load **value-identical**, `fit_msm` coefficients agree to **1.4e-11** (observed; the suite enforces rel **1e-6** — glm float noise from the ~1e-16 weight perturbation). |
| **(d)** | **DEPS / FALLBACK** | `Suggests: TrialEmulation, data.table`; backend defined conditionally in `.onLoad`; `data.table` use enabled via the `.datatable.aware` namespace flag (documented opt-in for `Suggests`-level data.table). No new Rust dep. |
| **maintainers** | **#243** | gravesti green-lit both **#1** (companion) and **#2** (`Suggests` fallback); sole constraint *"don't add to CRAN time"*. The detailed questions (interface / pin / contract) are still **unanswered**, so the documented assumptions stand: target the **`trial_sequence()` S4 interface**, pin **v0.0.4.11**, companion-expansion approach. Re-verify the thread before any follow-up. |

**Chunk-invariance** was confirmed separately: `TrialEmulation`'s stored frame is
byte-identical at `chunk_size = 500` vs `10` (globally `(id, period_new,
trial_period)`), so the companion's single Rust pass + re-sort matches regardless
of the user's `chunk_size`.

---

## Verification gauntlet

| Gate | Result |
|---|---|
| `te_datastore_tters` testthat suite (skips if `TrialEmulation` absent) | **8 tests / 58 assertions, 0 fail** — D1 conformance, D2 frame-equivalence (ITT unweighted/weighted + PP weighted), `read_expanded_data` period/subset, D3 load+sample+`fit_msm` parity, D4 AT fallback + error paths. |
| End-to-end through the real package (`pkgload::load_all`) | save_to_tters / expand_trials_tters / show / read / load / sample / fit_msm all green; AT falls back with a message. |
| `R CMD INSTALL` (debug) | succeeds; package + `.so` load, new R symbols exported. **Release INSTALL is unaffected — zero Rust changed ⇒ `.so` byte-identical to the Phase-10 release build.** |
| Existing `tters` tests | unchanged + green (no regression; the binding's `*_df` contract suite is untouched). |
| `cargo fmt --check` (binding) | clean (no Rust touched). |
| Core / contract / `fixtures` / `oracle` / `SPEC.md` | **untouched**; both `Cargo.lock`s **unchanged**. |

> Disk note: the dev host had ~13 GB free; a release `R CMD INSTALL` of the
> Polars-backed `.so` transiently needs ~25 GB, so it was not re-run here — but
> the phase changes **no Rust**, so the release build is identical to Phase 10's
> (already verified). All R-level integration is validated via `load_all` + the
> testthat suite + a debug install.

---

## Decisions & deviations

- **Companion expansion, not a storage backend.** `te_datastore` is the *handoff
  contract*; the value is the Rust expansion in front of it (the SEAM). The user
  calls `expand_trials_tters()` instead of `expand_trials()`.
- **Interface: the new S4 `trial_sequence()` path** (where `te_datastore` /
  `set_expansion_options` live), per our #243 reply. The legacy
  `data_preparation()` interface is out of scope.
- **Weights: read R's `wt` verbatim; accumulate in Rust.** Weight *models* are fit
  by `calculate_weights()` in R (statistical estimation). The companion feeds that
  per-period `wt` as the `tters` factor table, so Rust performs only the
  deterministic cumulative product — bit-exact to TE's `weight0 / wtprod`.
- **RNG: inherit the base `sample_expanded_data`** (do not override). Safest for
  parity — identical `do_sampling` over an identically-ordered read.
- **Row order: re-sort to the stored `index` order in R.** A cheap deterministic
  total sort; the only transform needed beyond the Rust expansion + the baseline
  covariate join.
- **`fit_msm` / sampling math / sandwich variance stay in R** — out of scope to
  reimplement, by contract.
- **R package version kept at `0.1.0`** (consistent with Phases 8–10, which added
  features without a bump; avoids interaction with the Phase-10 dist tooling).

### The one-line "Extending" pointer to offer the maintainers

> A community companion package, [`tters`](https://github.com/oldschoolcool2/rust-tte),
> provides a Rust + Polars `te_datastore` backend (`save_to_tters()`) and a drop-in
> `expand_trials_tters()` that runs the trial expansion in Rust while estimation
> stays in `TrialEmulation`; it is `Suggests`-level and bit-identical to the
> default path.

(Offer only if/when they ask — do not open a PR against `Causal-LDA/TrialEmulation`.)

---

## Deferred items

- **AT-estimand `dose` path.** `trial_sequence_AT` (`treatment_var = "dose"`,
  the computed `dose = cumA_new − dosesum + treat` column) is not yet mapped;
  `expand_trials_tters()` falls back to R for AT. ITT/PP parity ships first.
- **A formal vignette.** A worked example is in the README + this summary; a built
  `vignette("…")` can be added later (kept out now to avoid `VignetteBuilder`
  build-surface churn and a `Suggests`-conditional knit).
- **`crates.io` publish of `tte-expand`.** Still deferred (gated on engaging the
  TrialEmulation team), per the publishing memory. The companion installs via the
  Phase-10 r-universe / source path.
- **New-interface tracking.** If the maintainers answer #243 with a different
  preferred surface or a dev branch, re-pin and adapt.
- **Test staging.** The new testthat file is committed under `tests-staging/`
  (the agent guard blocks writes to `tests/`); a human `git mv`s it to
  `tests/testthat/` in a follow-up commit.
