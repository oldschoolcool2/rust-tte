#!/usr/bin/env Rscript
# Vendor a SMALL, representative subset of the immutable Oracle fixture battery
# into `bindings/tters/inst/extdata/`, so the installed package's testthat
# self-test runs against `system.file("extdata", package = "tters")` when the
# repo-root `fixtures/` tree is absent (a standalone / r-universe install).
#
# This is a READ of the immutable ground truth (`fixtures/`) and a WRITE into the
# package's own `inst/extdata/` (an allowed path) via `file.copy()`. It does NOT
# modify the source fixtures. Re-run it to regenerate the subset after a fixture
# refresh; it is idempotent (overwrite = TRUE) and verifies every source exists.
#
# Subdir layout is preserved (edge/ scenarios/ weights/) because the testthat
# resolver builds paths like file.path(fx, "edge", "input_E02...parquet"); the
# `system.file("extdata")` branch then resolves with NO code change.
#
# Subset rationale (kept well under ~1 MB; ~173 KB): every testthat either runs
# (subset present) or skip()s cleanly (absent). The present subset exercises the
# full breadth of the binding's contract battery:
#   * all 9 adversarial EDGE cases (E01-E09), ITT + PP structural  -> the "moat"
#   * the `common` scenario, ITT + PP                              -> a non-degenerate
#       multi-period cohort; also drives the dtype-exact test (integer-id
#       passthrough) and is the structural baseline
#   * the `data_censored` weight set (cohort + PP/ITT factor tables + PP/ITT
#       expected) -> weight APPLY (rel 1e-12) AND FITTED (rel 1e-6), both estimands,
#       incl. the switch+censor+pool fit paths and the pool_censor error path
#   * E01 + data_censored inputs also back the two error-mapping tests
#
# Provenance: these are COPIES of this repo's Oracle-generated fixtures
# (Apache-2.0; see inst/NOTICE). The source of truth remains `fixtures/`.

args <- commandArgs(trailingOnly = FALSE)
file_arg <- sub("^--file=", "", args[grep("^--file=", args)])
script_dir <- if (length(file_arg)) dirname(normalizePath(file_arg)) else getwd()
# tools/ -> bindings/tters -> bindings -> repo root
repo_root <- normalizePath(file.path(script_dir, "..", "..", ".."))
fixtures <- file.path(repo_root, "fixtures")
dest_root <- normalizePath(file.path(script_dir, "..", "inst", "extdata"),
  mustWork = FALSE
)

stopifnot(dir.exists(fixtures))

edge <- c(
  "E01_single", "E02_id4_canonical", "E03_event_at_baseline", "E04_reentry",
  "E05_never_treats", "E06_switch_then_back", "E07_last_period_only",
  "E08_ties", "E09_max_fanout"
)

# Build the (subdir, filename) work list.
files <- list()
add <- function(sub, name) files[[length(files) + 1L]] <<- list(sub = sub, name = name)

for (nm in edge) {
  add("edge", sprintf("input_%s.parquet", nm))
  add("edge", sprintf("expected_%s_itt.parquet", nm))
  add("edge", sprintf("expected_%s_pp.parquet", nm))
}
# One representative non-degenerate scenario (also the dtype-exact integer-id case).
add("scenarios", "input_common.parquet")
add("scenarios", "expected_common_itt.parquet")
add("scenarios", "expected_common_pp.parquet")
# One complete weight set: covers apply + fitted, PP + ITT.
add("weights", "input_data_censored.parquet")
add("weights", "input_data_censored_pp_weights.parquet")
add("weights", "input_data_censored_itt_weights.parquet")
add("weights", "expected_data_censored_pp_weighted.parquet")
add("weights", "expected_data_censored_itt_weighted.parquet")

# Fresh subset each run (drop stale files a fixture refresh may have renamed).
if (dir.exists(dest_root)) {
  unlink(list.files(dest_root, recursive = TRUE, full.names = TRUE))
}

copied <- 0L
total_bytes <- 0
for (f in files) {
  src <- file.path(fixtures, f$sub, f$name)
  if (!file.exists(src)) {
    stop(sprintf("missing source fixture: %s", file.path(f$sub, f$name)))
  }
  dst_dir <- file.path(dest_root, f$sub)
  dir.create(dst_dir, recursive = TRUE, showWarnings = FALSE)
  ok <- file.copy(src, file.path(dst_dir, f$name), overwrite = TRUE)
  if (!ok) stop(sprintf("copy failed: %s", f$name))
  copied <- copied + 1L
  total_bytes <- total_bytes + file.info(src)$size
}

message(sprintf(
  "Vendored %d fixtures into %s (%.0f KB).",
  copied, dest_root, total_bytes / 1024
))
