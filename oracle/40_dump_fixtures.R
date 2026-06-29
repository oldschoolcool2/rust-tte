# oracle/40_dump_fixtures.R
source("oracle/00_setup.R")

# Run the Oracle once and return ONLY the deterministic structural columns.
#
# ITT goes through the legacy data_preparation() path (unchanged from Phase 1).
# PP is handled separately by oracle_expand_pp(): the legacy
# data_preparation(estimand_type = "PP") path fits a switch-weight glm that
# errors ("Argument mu must be a nonempty numeric vector") on degenerate
# single-patient / control-only cohorts (E01, E03, E05, E07, E08, E09), so we
# take the censoring from the modern S4 expand_trials() path instead.
oracle_expand <- function(cohort, estimand) {
  if (estimand == "PP") return(oracle_expand_pp(cohort))
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

# Per-protocol artificial censoring. We do NOT use the legacy
# data_preparation(estimand_type = "PP") path (it fits a switch-weight glm that
# crashes on degenerate cohorts, see above). Instead:
#
#   1. Expand ITT via data_preparation() -> the canonical 6-column
#      STRUCTURAL_COLS frame with the correct input-derived dtypes and the
#      assigned_treatment column.
#   2. Take the authoritative PP censoring from the modern S4
#      trial_sequence("PP") |> ... |> expand_trials() path, which does the
#      structural expansion only (no glm) and therefore never crashes.
#   3. Keep exactly the ITT rows whose (id, trial_period, followup_time) key the
#      S4 PP design retains.
#
# This yields PP fixtures in the SAME 6-column schema/dtypes as the ITT fixtures
# (censoring manifests purely as missing follow-up rows; the S4 placeholder
# `weight` column is dropped, exactly as the ITT path drops its `weight`). On
# every retained PP row treatment == assigned_treatment by construction (PP keeps
# only the strictly-pre-deviation, adherent rows), so the reattached
# assigned_treatment is exact, not invented. Verified bit-for-bit against the
# legacy PP path on every cohort where that path runs (E02/E04/E06 + all 8
# scenarios) and against the S4 row-set everywhere.
oracle_expand_pp <- function(cohort) {
  itt  <- oracle_expand(cohort, "ITT")
  surv <- s4_pp_survivors(cohort)
  k <- function(d) paste(
    as.character(as.numeric(d$id)),
    as.integer(round(as.numeric(d$trial_period))),
    as.integer(round(as.numeric(d$followup_time))),
    sep = "|"
  )
  itt[k(itt) %in% k(surv), , drop = FALSE]
}

# Structural PP expansion via the S4 path (expansion only, no switch-weight glm).
# Returns the expanded data.table; only its (id, trial_period, followup_time)
# keys are consumed by oracle_expand_pp().
s4_pp_survivors <- function(cohort) {
  ts <- trial_sequence("PP")
  ts <- set_data(ts, data = cohort, id = "id", period = "period",
                 treatment = "treatment", outcome = "outcome", eligible = "eligible")
  ts <- set_expansion_options(ts, output = save_to_datatable(), chunk_size = 0)
  ts <- expand_trials(ts)
  as.data.frame(ts@expansion@datastore@data)
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
