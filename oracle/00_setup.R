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
