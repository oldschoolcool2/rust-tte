# oracle/40_dump_fixtures.R
source("oracle/00_setup.R")

# Run the Oracle once and return ONLY the deterministic structural columns.
oracle_expand <- function(cohort, estimand) {
  td <- tempfile("te_"); dir.create(td)
  on.exit(unlink(td, recursive = TRUE), add = TRUE)
  prep <- data_preparation(
    data = cohort, id = "id", period = "period", treatment = "treatment",
    outcome = "outcome", eligible = "eligible",
    estimand_type = estimand, outcome_cov = ~1,
    use_censor_weights = FALSE,                 # keep expansion free of glm
    data_dir = td, separate_files = FALSE, quiet = TRUE
  )
  expanded <- as.data.frame(prep$data)
  keep <- intersect(STRUCTURAL_COLS, names(expanded))
  if (!setequal(keep, STRUCTURAL_COLS))
    warning("STRUCTURAL_COLS mismatch for ", estimand, ": have {",
            paste(names(expanded), collapse=", "), "}")
  # deterministic ordering so byte-comparison is stable
  ord <- do.call(order, expanded[intersect(c("id","trial_period","followup_time"), keep)])
  expanded[ord, keep, drop = FALSE]
}

dump_fixture <- function(cohort, name, subdir, estimands = c("ITT","PP")) {
  out_dir <- file.path(OUT_ROOT, subdir); dir.create(out_dir, recursive = TRUE, showWarnings = FALSE)
  in_path <- file.path(out_dir, sprintf("input_%s.parquet", name))
  arrow::write_parquet(cohort, in_path)
  entries <- list(list(
    role = "input", path = in_path, sha256 = digest(file = in_path, algo = "sha256"),
    n_rows = nrow(cohort)
  ))
  for (est in estimands) {
    expected <- oracle_expand(cohort, est)
    ep <- file.path(out_dir, sprintf("expected_%s_%s.parquet", name, tolower(est)))
    arrow::write_parquet(expected, ep)
    entries[[length(entries)+1]] <- list(
      role = paste0("expected_", tolower(est)), path = ep,
      sha256 = digest(file = ep, algo = "sha256"), n_rows = nrow(expected)
    )
  }
  entries
}

write_manifest <- function(all_entries, path = file.path(OUT_ROOT, "MANIFEST.json")) {
  jsonlite::write_json(
    list(provenance = ORACLE_PROVENANCE, fixtures = all_entries),
    path, pretty = TRUE, auto_unbox = TRUE
  )
  message("Manifest -> ", path)
}
