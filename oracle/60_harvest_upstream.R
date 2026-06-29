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
