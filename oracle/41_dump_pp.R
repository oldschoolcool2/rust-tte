# oracle/41_dump_pp.R — regenerate the Phase-2 per-protocol fixtures.
#
# Writes expected_<case>_pp.parquet for the 9 edge cases + 8 scenarios, derived
# from the COMMITTED input_<case>.parquet (the exact bytes the Rust engine reads).
# Inputs and the ITT fixtures are left untouched; PP carries the same six
# STRUCTURAL_COLS as ITT (censoring = missing rows). The per-protocol censoring is
# taken from the S4 expand_trials() path inside oracle_expand_pp() (see
# 40_dump_fixtures.R) so it never hits the legacy data_preparation PP glm crash.
#
# Run from the repo root, AFTER 40_dump_fixtures.R carries the PP recipe:
#     Rscript oracle/41_dump_pp.R
source("oracle/40_dump_fixtures.R")  # oracle_expand() incl. the PP branch; libs + consts

CASES <- list(
  edge = c("E01_single", "E02_id4_canonical", "E03_event_at_baseline", "E04_reentry",
           "E05_never_treats", "E06_switch_then_back", "E07_last_period_only",
           "E08_ties", "E09_max_fanout"),
  scenarios = c("common", "rare_event", "ultra_rare_event", "rare_initiation",
                "high_switching", "heavy_censoring", "short_followup", "strong_confounding")
)

summ <- data.frame()
for (sub in names(CASES)) {
  for (name in CASES[[sub]]) {
    in_path <- file.path(OUT_ROOT, sub, sprintf("input_%s.parquet", name))
    cohort  <- as.data.frame(arrow::read_parquet(in_path))
    pp      <- oracle_expand(cohort, "PP")
    ep      <- file.path(OUT_ROOT, sub, sprintf("expected_%s_pp.parquet", name))
    arrow::write_parquet(pp, ep)
    summ <- rbind(summ, data.frame(
      subdir = sub, case = name, pp_rows = nrow(pp),
      sha256 = digest::digest(file = ep, algo = "sha256")
    ))
  }
}
cat("\n==== PP fixtures written under '", OUT_ROOT, "' ====\n", sep = "")
print(summ, row.names = FALSE)
