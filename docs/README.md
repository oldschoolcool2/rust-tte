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

### `001-initial-ideations/`

| File | What it is |
|---|---|
| [`001-project-memories.md`](001-initial-ideations/001-project-memories.md) | High-level orientation: what the project is and the key decisions. |
| [`002-research-simulation-inputs.md`](001-initial-ideations/002-research-simulation-inputs.md) | Literature review of simulation inputs (DGPs, coefficients, known-truth estimands) and validation criteria across three tiers. |
| [`003-project-plan.md`](001-initial-ideations/003-project-plan.md) | The authoritative roadmap: thesis, scope, ADRs, phases, agent loop protocol, copy-paste prompts. |
| [`004-prework-fixtures.md`](001-initial-ideations/004-prework-fixtures.md) | Phase-0 made concrete: the R Oracle scripts that produce the fixture battery and the three-tier validation map. |
