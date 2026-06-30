# Phase 10 — Distribution & Self-Contained Installability for `tters`: Completion Summary & Sign-off

**Status: ✅ Implemented. The `tters` R package can now be built and installed —
and run its full testthat battery — WITHOUT the surrounding monorepo. A
self-contained source tarball (core crate + every Cargo dependency vendored)
installs fully offline with the repo-root `crates/tte-expand` and `fixtures/`
unreachable, and the battery runs against a small vendored `inst/extdata` fixture
subset (444 pass / 0 fail) instead of `skip()`-ping. The verified core stays
bit-for-bit unchanged; the committed working tree keeps its `path` dependency, so
the monorepo dev workflow, both lockfiles, and `make verify` are byte-identical and
untouched.**
Date: 2026-06-30.

This phase closes the long-standing distribution cluster deferred since Phase 4
(*"`inst/extdata` fixture vendoring"* and *"CRAN/r-universe distribution &
path-dependency vendoring — `cargo vendor` does not vendor path deps and `R CMD
build` only tarballs files under `bindings/tters/`"*). It is packaging/engineering,
not new engine logic: no core change, no surface change, no new Rust dependency.
Work is confined to `bindings/tters/**`, a repo-level `tools/r-universe/`, and this
doc folder.

## The mission (recap)

`tters`'s Rust crate path-deps the core at `../../../../crates/tte-expand`, which
resolves ONLY in the monorepo; its self-test reads the repo-root `fixtures/`.
Neither survives `R CMD build` (which tarballs only files under `bindings/tters/`)
or a checkout of just the package. Phase 10 closes both gaps so `tters` can ship via
r-universe / a source tarball — the prerequisite for the upstream `te_datastore`
companion-backend outreach (Track C).

## What was implemented

Edits are confined to `bindings/tters/**`, the repo-level `tools/r-universe/`, and
this `docs/012-…` folder. The verified core (`crates/tte-expand/**`), the contract
suite, `fixtures/`, `oracle/`, and `SPEC.md` are **untouched**. **The committed
`bindings/tters/src/rust/Cargo.toml` and `Cargo.lock` are byte-identical to Phase 9
— the `path` dependency stays the dev default.**

### D1 — Self-contained self-test (`inst/extdata`)

- **`bindings/tters/inst/extdata/{edge,scenarios,weights}/`** — a ~170 KB, 35-file
  representative SUBSET of the immutable Oracle battery, vendored as COPIES (the
  canonical source remains `fixtures/`). The subset is chosen so every testthat
  either runs (subset present) or `skip()`s cleanly (absent), and the present
  subset exercises the full breadth of the contract: all 9 adversarial **edge**
  cases (E01–E09) ITT+PP structural; the **`common`** scenario ITT+PP (also the
  integer-id dtype-exact case); and the **`data_censored`** weight set (cohort +
  PP/ITT factor tables + PP/ITT expected) — weight **apply** (rel 1e-12) and
  **fitted** (rel 1e-6), both estimands incl. the switch+censor+pool path, plus the
  two error-mapping tests.
- **`bindings/tters/tools/vendor-extdata.R`** — the regeneration helper. It READS
  `fixtures/` and WRITES into `inst/extdata/` via R `file.copy()` (the source
  fixtures are never modified); idempotent, verifies every source exists.
- **No resolver code change.** The three testthat files already resolve
  `$TTERS_FIXTURE_DIR` → repo-root `fixtures/` (walk-up) → `system.file("extdata",
  package = "tters")` → `skip()`. Vendoring the subset under the `edge/scenarios/
  weights` layout makes the existing `system.file` branch fire on a standalone
  install; the repo-root `fixtures/` stays the default in the monorepo (no
  regression). Confirmed working when installed (see Verification).

### D2 — Self-contained source build (the path-dep blocker)

The committed manifest keeps the **`path`** dep (dev default, lock unchanged). The
distributable **git** form is **synthesized at distribution time** by a scripted,
single-source-preserving step — never committed:

- **`bindings/tters/tools/rewrite-core-dep.sh`** — rewrites the `tte-expand` path
  dep → `git`+pinned-`rev` dep IN PLACE on a throwaway tree, and **surgically**
  inserts the matching `source = "git+<url>?rev=<rev>#<rev>"` line into the existing
  `Cargo.lock` stanza, touching nothing else — a `--locked`-clean change that
  preserves every pinned transitive version (no `cargo generate-lockfile`
  re-resolution, which was observed to drift a transitive).
- **`bindings/tters/.prepare`** — the **r-universe** build hook. r-universe clones
  the whole monorepo and runs `$PKGDIR/.prepare` *while the siblings are still on
  disk*; this resolves the rev dynamically (`git rev-parse HEAD`) and rewrites the
  path dep → `git` dep on the canonical **public** URL at that exact commit. The
  subdir is then self-contained for `R CMD build`; r-universe's build-time network
  fetches the core + crates.io deps (no committed vendor tree needed there).
- **`bindings/tters/tools/build-offline-tarball.sh`** — the **offline / source**
  counterpart. On a throwaway copy (committed tree untouched) it rewrites path →
  git (a local `file://` bare clone by default, so vendoring needs no network; pass
  the public URL for a release artifact), runs `cargo vendor` (which DOES vendor the
  git core and **bakes its workspace inheritance** — `polars = { workspace = true }`,
  edition, license — into a concrete manifest, so the **Polars feature set is
  identical by construction**, preserving bit-for-bit determinism), packs
  `src/rust/vendor.tar.xz` + `vendor-config.toml`, and `R CMD build`s a
  self-contained tarball that the existing Makevars vendor plumbing installs with
  no network.

Why **git-dep**, not an in-package copy: the core crate inherits from the workspace
(`polars = { workspace = true }`, edition, license, …). A git dep checks out the
whole repo, so cargo resolves that inheritance natively (the exact Polars features
preserved). An in-package copy would require a hand-written manifest de-inheritance
— a determinism hazard the project guards against. Publishing `tte-expand` to
crates.io (the cleanest cargo story) stays **deferred** until the upstream
maintainers are engaged.

### D3 — r-universe registration

- **`tools/r-universe/packages.json`** — the registry entry
  (`{package: tters, url: …/rust-tte, subdir: bindings/tters}`) to push to a
  `oldschoolcool2.r-universe.dev` registry repo.
- **`tools/r-universe/README.md`** — the empirically-validated build model (full
  clone → `.prepare` → subdir `R CMD build` → network compile) and the go-live
  steps (make the repo public; create + push the registry repo). These public,
  account-level GitHub actions are documented for the owner to perform as the
  capstone.
- **CI (`.github/workflows/r-binding.yml`)** — a new `standalone-install` job
  (path-filtered, `CARGO_PROFILE_*_DEBUG=0`) builds the self-contained tarball,
  installs it OFFLINE with the monorepo unreachable, and runs the battery against
  `inst/extdata` — continuous regression coverage for the distribution path,
  alongside the existing monorepo release install + full-battery job.

### D4 — Distribution metadata & hygiene

- **`bindings/tters/inst/NOTICE`** — carries the upstream TrialEmulation Apache-2.0
  attribution (the Oracle) and the provenance of the vendored example data (this
  project's Oracle-generated fixtures, Apache-2.0). Installed (`inst/`), so a
  standalone package ships it; `R CMD check`-clean (no non-standard top-level file).
- **`.gitignore`** (binding `src/`): the generated `vendor.tar.xz` /
  `vendor-config.toml` / unpacked `vendor/` are **never committed** (a ~600 MB
  release artifact, regenerated on demand).
- **`.Rbuildignore`**: excludes `.prepare` (an r-universe hook, not a package file),
  the unpacked `vendor/`, and `tests-staging/` from the shipped tarball; the shipped
  `vendor.tar.xz` is kept.
- **CRAN is explicitly NOT a target** — the vendored Polars tree (~600 MB raw, 369
  crates) busts CRAN's 5 MB limit (the same reason the `polars` R package lives on
  r-universe, not CRAN).

## VERIFY-FIRST findings (empirical, established + signed off BEFORE building)

| Question | Finding |
|---|---|
| **The break (reproduced)** | `R CMD build bindings/tters` → 59 KB tarball with the core crate ABSENT; installing it fails at cargo resolution: `failed to load manifest for dependency 'tte-expand' … No such file or directory`. The `../../../../` path escapes the tarball. |
| **r-universe build model** (primary sources, adversarially verified) | Full `git clone` of the monorepo → a `$PKGDIR/.prepare` hook runs while siblings are on disk → `R CMD build` tarballs the **subdir only** (`crates/` EXCLUDED) → all binary jobs compile that tarball ⇒ **the path dep is NOT reachable for any distributable build.** Runners have **build-time network** (the cargo shim execs real cargo, no `--offline`) ⇒ a **git dep is fetched at build time; no committed vendor tree needed**. r-universe sets `MY_UNIVERSE` (not `NOT_CRAN`). |
| **Strategy (signed off)** | Path → **git+pinned-rev**, **path kept as the committed dev default**; the git form + offline vendor tree are synthesized at dist time. Single-source (no committed core copy); bit-exact-safe (git checkout resolves workspace inheritance natively → identical Polars features); lock changes by exactly one line at dist time. The in-package copy was rejected (fragile manifest de-inheritance = determinism hazard); crates.io publish deferred. CONFIRMED cargo gotcha: a `.cargo/config.toml` `paths` override only matches crates.io-published crates, NOT a git-only dep — so the dev default stays the literal path dep, not an override. |
| **`inst/extdata` subset** | ~170 KB / 35 files: 9 edge cases (input+itt+pp) + `common` scenario (input+itt+pp) + the `data_censored` weight set. Covers ITT/PP structural, dtype-exactness, apply (1e-12), fitted (1e-6), both error paths; rides the existing `system.file("extdata")` resolver with NO code change. |
| **Footprint (signed off)** | `cargo vendor` tree = 638 MB raw (369 crates, Windows-target dominated) → **gitignored, generated on release/CI, never committed**. The committed `inst/extdata` (~170 KB) is the only in-tree data. |
| **Repo visibility** | The repo is going **public** (owner-confirmed; gitleaks-guarded since Phase 0) — the prerequisite for r-universe and git-dep source installs. |

## Verification performed (2026-06-30, Rust 1.95.0, R 4.3.3)

| Check | Result |
|---|---|
| **Decisive: monorepo-absent install passes the battery** — `tools/build-offline-tarball.sh` → `R CMD INSTALL` (`CARGO_NET_OFFLINE=true`, repo `crates/`+`fixtures/` unreachable) | ✅ `* DONE (tters)`; **576 crates compiled fully offline from `vendor.tar.xz`** (incl. the git-vendored core); 0 errors |
| **Equivalence gate** — testthat battery vs `inst/extdata` (repo fixtures unreachable) | ✅ **444 pass / 0 fail / 0 warn / 44 skip** across all 4 test files; structural columns `expect_identical` ⇒ byte-identical output; the 44 skips are the intentionally-unvendored larger weight fixtures |
| Committed `src/rust/Cargo.toml` + `Cargo.lock` untouched (path dep preserved) | ✅ pristine (`git status` empty) |
| Verified core byte-identical (vendored git-core `src/` vs `crates/tte-expand/src`) | ✅ `diff -rq` identical |
| `cargo fmt --all --check` (binding) | ✅ clean |
| `cargo deny check` (binding tree, repo `deny.toml`) | ✅ advisories / bans / licenses / sources ok (path-dep tree ⇒ no `allow-git` needed) |
| `cargo clippy --all-targets --all-features -- -D warnings` (binding) | ✅ clean |
| `cargo test` (binding Rust crate) | ✅ 3 passed (the Phase-9 bit-reinterpret unit tests) |
| Root `make verify` (test + certificate) | ✅ integrity 47/47 fixtures match manifest; spot-checks pass |
| `R CMD INSTALL bindings/tters` (monorepo, debug) + **full** battery (repo `fixtures/`) | ✅ `* DONE`; **817 pass / 0 fail / 0 skip** across all 4 test files (no regression; complete fixture set) |
| `R CMD INSTALL bindings/tters` (monorepo, release, LTO) | ✅ covered by CI (`r-binding.yml` runs the release install + full battery on this PR; the binding's Rust build inputs are byte-identical to Phase-9's verified release) |
| `R CMD check` on the self-contained tarball | ✅ **Status: 3 NOTEs, 0 ERROR / 0 WARNING** — all benign: (i) installed size 353 MB (the documented vendored-Polars NOTE), (ii) `bit64` in Imports not imported (the intended Phase-9 decision — used by the Rust side, not the R namespace), (iii) line endings in `src/rust/target/debug/build/*/flag_check.c` (debug-only cargo build-script artifacts; release `rust_clean` removes `target/`) |
| New CI `standalone-install` job (`r-binding.yml`) | ✅ added — builds the self-contained tarball + installs it offline (monorepo unreachable) + runs the battery vs `inst/extdata`; path-filtered, `CARGO_PROFILE_*_DEBUG=0` |
| Both lockfiles committed (root + binding) UNCHANGED | ✅ |

## Decisions / deviations recorded

- **Path dep stays the committed dev default; git form synthesized at dist.** This
  honors "keep the monorepo dev workflow working (path dep still the default)" AND
  keeps both lockfiles byte-identical. The git form lives only in throwaway dist
  artifacts (`.prepare`'s r-universe clone; `build-offline-tarball.sh`'s temp copy).
- **git-dep over in-package copy — a determinism decision.** The core's workspace
  inheritance (esp. `polars = { workspace = true }`) is resolved natively by a git
  checkout; an in-package copy would hand-transcribe the Polars feature set and
  could drift, perturbing bit-exact output. Verified: `cargo vendor` bakes the
  inheritance into a concrete vendored manifest, and the vendored core `src/` is
  byte-identical to `crates/tte-expand/src`.
- **Single-source preserved.** No committed duplicate of the core; the only in-tree
  data is the ~170 KB `inst/extdata` subset (COPIES, regenerable from `fixtures/`
  via `tools/vendor-extdata.R`); the offline vendor tree is generated on demand.
- **Footprint.** The ~600 MB vendor tree is gitignored and release-only; CRAN is
  not targeted (the Polars tree busts the 5 MB limit — the r-polars precedent).
- **No core/contract change, no new Rust dependency, both lockfiles unchanged.**

## Deferred to later phases / owner actions

- **Go-live (owner, public/outward-facing):** flip the repo to public
  (`gh repo edit … --visibility public`) and create + push the
  `oldschoolcool2.r-universe.dev` registry repo with `tools/r-universe/packages.json`.
  Documented in `tools/r-universe/README.md`; left to the owner as the capstone.
- **Publishing `tte-expand` to crates.io** — the cleanest cargo distribution story,
  but gated on engaging the upstream maintainers (per the rustification roadmap).
- **Track C — the `te_datastore` companion backend** (the next track) — now
  unblocked by this phase: `tters` is installable outside the monorepo, so it can be
  registered as an upstream expansion backend.
- **Carried over unchanged:** zero-copy Arrow stream (declined; needs `unsafe`),
  the fixture-gated weight-model shapes, and the *robust variance / MSM stays in R*
  boundary.
