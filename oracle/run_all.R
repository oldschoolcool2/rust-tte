# oracle/run_all.R — regenerate the full fixture set deterministically.
source("oracle/20_scenarios.R")
source("oracle/30_edge_cases.R")
source("oracle/40_dump_fixtures.R")
source("oracle/50_golden_pipeline.R")

entries <- list()

# Tier 1a — simulated scenarios (exact-match)
for (nm in names(SCENARIOS)) {
  if (nm == "large_scale") next                      # benchmark only, not a unit fixture
  cohort <- build_scenario(nm)
  entries <- c(entries, dump_fixture(cohort, nm, subdir = "scenarios"))
}

# Tier 1b — hand-authored edge cases (exact-match)
for (nm in names(EDGE_CASES)) {
  entries <- c(entries, dump_fixture(EDGE_CASES[[nm]], nm, subdir = "edge"))
}

# Tier 2 — whole-pipeline goldens (tolerance-based)
data(data_censored); golden_pipeline(data_censored, "data_censored")
data(trial_example); golden_pipeline(trial_example, "trial_example")

write_manifest(entries)
cat("\nDone. Fixtures under '", OUT_ROOT, "'. CI must regenerate and diff sha256.\n", sep = "")
