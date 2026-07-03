# Project Overview — Rust-ifying Causal-Inference Tooling for Epidemiology

*Project overview and design rationale (as of June 29, 2026).*

1. This project modernizes causal inference / epidemiology tooling for healthcare research by porting performance-critical parts of R epi packages to Rust, using Claude Code as an autonomous (fixture-driven) coding loop.

2. Flagship effort **'tte-expand'**: a verified Rust+Polars backend for the data-expansion stage of sequential target trial emulation, validated bit-exact against the TrialEmulation R package (Causal-LDA, Apache-2.0, maintained by Isaac Gravestock/Roche and Li Su/MRC-BSU).

3. Core method is the **'fixture-driven strangler pattern'**: use the R package as an immutable Oracle to generate Parquet test fixtures; Claude Code writes idiomatic Rust to match them; the agent is forbidden from modifying tests, fixtures, or tolerances.

4. Key architectural decisions for tte-expand: staged tolerance (bit-exact for the deterministic expansion, documented numeric tolerance only for statistical estimation); Parquet not CSV for fixtures; Polars engine; expose to R via extendr; keep glm/parglm and sandwich/robust variance in R rather than reimplementing them in Rust.

5. tte-expand v1 scope is sequential trial expansion **ONLY** (Hernán 2008 / Gran 2010 / Danaei 2013) — not the clone-censor-weight grace-period design (which is upstream TrialEmulation issue #115, a separate later contribution) and not a statistical solver. The distinction between sequential TE and CCW is maintained throughout this project.

6. Contribution plan for tte-expand: ship as a companion extendr R package, produce a computational-reproducibility certificate (bit-exact equivalence to the CRAN gold standard), collaborate with the maintainers, and write up via JOSS / preprint with an ENCePP / RWE-reproducibility framing. The project is a verified high-performance backend developed with the maintainers, not a replacement.

7. Two planning docs already produced for tte-expand: **'tte-expand-project-plan.md'** (roadmap, phased milestones, agent loop rules + copy-paste prompts, drop-in CLAUDE.md) and **'tte-expand-prework-fixtures.md'** (R scripts for simulation-based + harvested Oracle fixture generation, three-tier validation map). Next artifacts to build: SPEC.md (R-free behavioural spec) and the empty cargo test harness loading the Parquet fixtures.
