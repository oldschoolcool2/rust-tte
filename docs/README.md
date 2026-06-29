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
