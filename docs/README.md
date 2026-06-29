# Documentation

Design docs, research, and decision records for `tte-expand`.

## Convention

Documentation is organised into **numbered topic folders** and **numbered files**:

```
docs/
└── NNN-short-description/        # a numbered topic folder
    ├── 001-first-document.md     # numbered, ordered Markdown files
    ├── 002-second-document.md
    └── ...
```

- Folders: `NNN-short-description/` — `NNN` is a zero-padded sequence (`001`,
  `002`, …); the description is kebab-case.
- Files: `NNN-short-description.md` inside a folder, numbered in reading order.
- Numbers are **append-only** — never renumber an existing folder/file; add the
  next number.

## Index

| Folder | Contents |
|---|---|
| [`001-initial-ideations/`](001-initial-ideations/) | The originating research and plan: project memories, the simulation-inputs/validation literature review, the phased project plan, and the executable Phase-0 fixture-generation pre-work. |
| [`002-phase-0-scaffold/`](002-phase-0-scaffold/) | Phase-0 completion summary, verification results, decisions/deviations, and the remaining human sign-off items. |
| [`003-phase-1-itt-expansion/`](003-phase-1-itt-expansion/) | Phase-1 ITT engine: the self-join algorithm, the input-derived dtype rules, fixtures generated, bit-exact verification, and the deferred PP/edge-case sign-off items. |
| [`004-phase-2-pp-censoring/`](004-phase-2-pp-censoring/) | Phase-2 per-protocol engine: first-deviation artificial censoring (`cum_max`/window), the `PP = ITT ∩ S4-survivors` fixture recipe, the no-flag-column schema decision, bit-exact verification, and the ITT-unchanged proof. |
| [`005-phase-3-weight-application/`](005-phase-3-weight-application/) | Phase-3 weight application: the join + cumulative-product (`cum_prod`/window) design, the per-`(id, period)` factor table recovered from the Oracle weights, the `STRUCTURAL_COLS_WEIGHTED` schema, the 1e-12 tolerance, verification within tolerance, and the ITT/PP-unchanged proof. |
| [`006-phase-4-extendr-binding/`](006-phase-4-extendr-binding/) | Phase-4 extendr binding (`tters`): the FFI shim exposing estimand selection + weighted expansion to R, the regenerated extendr wrappers / NAMESPACE / man, the toolchain & version reconciliation (extendr 0.9 / rextendr 0.5 / R 4.3.3, MSRV bumped to 1.95), the `R CMD INSTALL` + testthat round-trip reproducing the full battery, and the root-workspace-unchanged proof. |

### `001-initial-ideations/`

| File | What it is |
|---|---|
| [`001-project-memories.md`](001-initial-ideations/001-project-memories.md) | High-level orientation: what the project is and the key decisions. |
| [`002-research-simulation-inputs.md`](001-initial-ideations/002-research-simulation-inputs.md) | Literature review of simulation inputs (DGPs, coefficients, known-truth estimands) and validation criteria across three tiers. |
| [`003-project-plan.md`](001-initial-ideations/003-project-plan.md) | The authoritative roadmap: thesis, scope, ADRs, phases, agent loop protocol, copy-paste prompts. |
| [`004-prework-fixtures.md`](001-initial-ideations/004-prework-fixtures.md) | Phase-0 made concrete: the R Oracle scripts that produce the fixture battery and the three-tier validation map. |

### `002-phase-0-scaffold/`

| File | What it is |
|---|---|
| [`001-phase-0-summary.md`](002-phase-0-scaffold/001-phase-0-summary.md) | Phase-0 sign-off: what each agent built, the verification (compiles / clippy / fmt / test green), decisions (Polars 0.54.4, MSRV 1.88, `dtype-categorical`), and remaining human sign-off. |

### `003-phase-1-itt-expansion/`

| File | What it is |
|---|---|
| [`001-phase-1-summary.md`](003-phase-1-itt-expansion/001-phase-1-summary.md) | Phase-1 sign-off: the ITT self-join engine, the input-derived dtype contract, the fixtures generated from the Oracle, bit-exact verification (13 fixtures + invariants, all gates green), the two Oracle bugs found, and the deferred PP / `E04`–`E09` / golden items. |

### `004-phase-2-pp-censoring/`

| File | What it is |
|---|---|
| [`001-phase-2-summary.md`](004-phase-2-pp-censoring/001-phase-2-summary.md) | Phase-2 sign-off: per-protocol first-deviation censoring (`Estimand::PerProtocol`, `cum_max` over `(id, trial_period)` ordered by `followup_time`), the `PP = ITT ∩ S4-survivors` fixture recipe, the no-flag-column / ITT-consistent schema decision, bit-exact verification across 17 PP fixtures + the monotone-censoring invariant, and the ITT-unchanged proof. |

### `005-phase-3-weight-application/`

| File | What it is |
|---|---|
| [`001-phase-3-summary.md`](005-phase-3-weight-application/001-phase-3-summary.md) | Phase-3 sign-off: weight application (`apply_weights` / `expand_weighted_parquet`) as a join of the per-`(id, period)` factor + a `cum_prod` window over `(id, trial_period)` ordered by `followup_time`, the legacy-path weight fixtures + factor tables (recovered as the trial-invariant ratio), the `STRUCTURAL_COLS_WEIGHTED` schema, the relative-1e-12 tolerance, the adversarially-verified cumulative-product decision, verification within tolerance across 5 fixtures, and the ITT/PP-unchanged proof. |

### `006-phase-4-extendr-binding/`

| File | What it is |
|---|---|
| [`001-phase-4-summary.md`](006-phase-4-extendr-binding/001-phase-4-summary.md) | Phase-4 sign-off: the `tters` extendr shim (`expand_parquet` / `expand_weighted_parquet` with estimand selection + faithful `ExpandError`→R error mapping), the ergonomic `expand_trial` / `expand_trial_weighted` wrappers, the regenerated extendr wrappers / NAMESPACE / man, the VERIFY-FIRST toolchain & version reconciliation (extendr-api 0.9.0, rextendr 0.5.0, R 4.3.3; MSRV bumped 1.71→1.95 for Polars), the `R CMD INSTALL` (debug + release) + testthat round-trip reproducing the full battery (structural exact + `weight` within 1e-12), and the root-workspace-unchanged proof. |
