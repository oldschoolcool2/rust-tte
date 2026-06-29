# tters — R binding for `tte-expand` (extendr)

`tters` is the R companion package that exposes the `tte-expand` Rust core crate
to R via [extendr](https://extendr.github.io/). It is the **Phase 4** deliverable
(see [`../../ROADMAP.md`](../../ROADMAP.md)); the files here are a **scaffold** and
the engine they call is not yet implemented.

## How it fits together

```
bindings/tters/
├── DESCRIPTION, NAMESPACE          # R package metadata
├── R/                              # R wrappers (extendr-wrappers.R is generated)
├── configure[.win], cleanup[.win]  # R CMD INSTALL build hooks
├── tools/                          # config.R / msrv.R (toolchain checks)
└── src/
    ├── Makevars.in, Makevars.win.in, entrypoint.c, tters-win.def
    └── rust/                       # a DETACHED Cargo workspace
        ├── Cargo.toml              # extendr-api + path dep on ../../../../crates/tte-expand
        └── src/lib.rs              # #[extendr] FFI shims
```

The Rust crate under `src/rust/` declares an **empty `[workspace]` table**, making
it its own workspace root with its own `Cargo.lock`. This is required so
`R CMD INSTALL` builds a self-contained, reproducible (CRAN-vendorable) crate. It
is excluded from the repo-root workspace (`exclude = ["bindings/tters"]` in the
root `Cargo.toml`) and is therefore **not** linted by the main CI `--workspace`
clippy job — extendr's macro-expanded FFI would otherwise trip pedantic lints.

## Regenerating with rextendr

These files mirror what [`rextendr`](https://extendr.github.io/rextendr/) produces.
To (re)scaffold or refresh after changing the Rust signatures, from this directory
in R:

```r
# install.packages("rextendr")
rextendr::document()        # regenerates R/extendr-wrappers.R + NAMESPACE
# or, to (re)create the binding scaffold from scratch:
# rextendr::use_extendr()
```

> **Verify pinned versions before relying on this scaffold.** `extendr-api` is
> pinned to `0.9` and `Config/rextendr/version` to `0.5.0` from research that
> could not be confirmed against crates.io offline. Run `rextendr::document()`
> and `cargo update` (with network) to reconcile to the installed versions.

## Caveats for distribution

- **Path dependency vs. CRAN.** `cargo vendor` does not vendor path dependencies,
  and `R CMD build` only tarballs files under `bindings/tters/`. Before any CRAN
  submission, either publish `tte-expand` to a registry and switch to a version
  dependency, or add a build step that copies `crates/tte-expand/` into the
  package. The vendored Polars tree is also large; r-universe / source install is
  the realistic channel.
- **Two lockfiles.** Commit both the root `Cargo.lock` and
  `src/rust/Cargo.lock` (Dependabot tracks the latter at
  `/bindings/tters/src/rust`).
