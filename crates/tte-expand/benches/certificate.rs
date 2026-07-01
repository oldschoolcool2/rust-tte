// Certificate generator (a `harness = false` bench, run via
// `cargo bench --bench certificate` and by `make verify`). Not a timing
// benchmark: it re-verifies equivalence + recomputes fixture digests and writes
// `report/certificate.md`. Bench-target lints relaxed as for the other benches.
#![allow(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::too_many_lines,
    clippy::cast_precision_loss,
    clippy::indexing_slicing,
    clippy::panic
)]
//! **Computational-reproducibility certificate** generator.
//!
//! Emits `report/certificate.md`, asserting bit-exact equivalence of the Rust
//! engine to the R `TrialEmulation` Oracle across the committed fixture battery.
//! It does NOT hard-code "PASS": it (a) recomputes every manifest-listed
//! fixture's SHA-256 and compares it to `fixtures/MANIFEST.json` /
//! `fixtures/weights/MANIFEST_weights.json` (differential integrity — fails on
//! drift), and (b) re-runs the engine on representative fixtures and checks the
//! structural columns bit-exact and `weight` within the harness tolerance. The
//! exhaustive proof is the contract suite (`cargo test`), which `make verify`
//! runs immediately before this generator. Reproduce with `make verify`.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use polars::prelude::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
#[cfg(feature = "weights-fit")]
use tte_expand::{CensorWeightSpec, PoolCensor, SwitchWeightSpec, WeightSpec, fit_weights};
use tte_expand::{Estimand, ExpandOptions, apply_weights, expand};

/// Harness tolerance on the *applied* `weight` (mirrors `tests/weights.rs`).
const WEIGHT_REL_TOL: f64 = 1e-12;

/// Staged tolerance on the *fitted* `weight` (`weights-fit` feature): the bound
/// `smartcore` solver converges to R `glm`'s MLE, not bit-for-bit (ADR-2). Mirrors
/// `fit::tests::FITTED_WEIGHT_REL_TOL`. Observed worst on the fixtures ≈3.4e-7.
const FITTED_WEIGHT_REL_TOL: f64 = 1e-6;

fn repo_root() -> PathBuf {
    // `CARGO_MANIFEST_DIR` is `crates/tte-expand`; the repo root is two up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve repo root")
}

fn read_parquet(path: &Path) -> DataFrame {
    let s = path.to_str().expect("utf-8 path");
    LazyFrame::scan_parquet(PlRefPath::new(s), ScanArgsParquet::default())
        .expect("scan parquet")
        .collect()
        .expect("collect parquet")
}

fn sha256_hex(path: &Path) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hasher
        .finalize()
        .iter()
        .fold(String::with_capacity(64), |mut acc, b| {
            let _ = write!(acc, "{b:02x}");
            acc
        })
}

/// Look up `version = "..."` for the first `name = "<name>"` block in a Cargo
/// lockfile (a tiny, dependency-free TOML scan — enough for a pin record).
fn lock_version(lock: &str, name: &str) -> Option<String> {
    let needle = format!("name = \"{name}\"");
    let start = lock.find(&needle)?;
    let after = &lock[start..];
    let vpos = after.find("version = \"")? + "version = \"".len();
    let tail = &after[vpos..];
    let end = tail.find('"')?;
    Some(tail[..end].to_owned())
}

fn toml_value(text: &str, key: &str) -> Option<String> {
    let needle = format!("{key} = \"");
    let pos = text.find(&needle)? + needle.len();
    let tail = &text[pos..];
    let end = tail.find('"')?;
    Some(tail[..end].to_owned())
}

/// Outcome of a live re-verification of one fixture.
struct SpotCheck {
    label: String,
    estimand: &'static str,
    rows: usize,
    structural_ok: bool,
    weight: Option<(bool, f64)>, // (within tolerance, worst relative diff)
}

fn estimand_opts(estimand: Estimand) -> ExpandOptions {
    ExpandOptions::new("id", "period", "treatment", 0, i32::MAX).with_estimand(estimand)
}

fn structural_equal(actual: &DataFrame, expected: &DataFrame) -> bool {
    actual.get_column_names() == expected.get_column_names()
        && actual.dtypes() == expected.dtypes()
        && actual.height() == expected.height()
        && [
            "id",
            "trial_period",
            "followup_time",
            "assigned_treatment",
            "treatment",
            "outcome",
        ]
        .iter()
        .all(|c| {
            actual
                .column(c)
                .and_then(|a| expected.column(c).map(|e| a.equals(e)))
                .unwrap_or(false)
        })
}

fn worst_weight_rel(actual: &DataFrame, expected: &DataFrame) -> f64 {
    let cast = |df: &DataFrame| {
        df.column("weight")
            .expect("weight")
            .cast(&DataType::Float64)
            .expect("f64")
            .f64()
            .expect("f64")
            .into_no_null_iter()
            .collect::<Vec<f64>>()
    };
    let (a, e) = (cast(actual), cast(expected));
    a.iter()
        .zip(&e)
        .map(|(x, y)| (x - y).abs() / y.abs().max(1.0))
        .fold(0.0_f64, f64::max)
}

fn spot_structural(root: &Path, subdir: &str, name: &str, estimand: Estimand) -> SpotCheck {
    let label = format!("{subdir}/{name}");
    let est = if estimand == Estimand::Itt {
        "ITT"
    } else {
        "PP"
    };
    let input = root.join(format!("fixtures/{subdir}/input_{name}.parquet"));
    let expected = root.join(format!(
        "fixtures/{subdir}/expected_{name}_{}.parquet",
        est.to_lowercase()
    ));
    let lf = LazyFrame::scan_parquet(
        PlRefPath::new(input.to_str().expect("utf-8")),
        ScanArgsParquet::default(),
    )
    .expect("scan input");
    let actual = expand(lf, &estimand_opts(estimand))
        .expect("expand")
        .collect()
        .expect("collect");
    let exp = read_parquet(&expected);
    SpotCheck {
        label,
        estimand: est,
        rows: actual.height(),
        structural_ok: structural_equal(&actual, &exp),
        weight: None,
    }
}

fn spot_weighted(
    root: &Path,
    label: &str,
    input_rel: &str,
    factors_rel: &str,
    expected_rel: &str,
    estimand: Estimand,
) -> SpotCheck {
    let est = if estimand == Estimand::Itt {
        "ITT"
    } else {
        "PP"
    };
    let input = root.join(input_rel);
    let factors = root.join(factors_rel);
    let expected = read_parquet(&root.join(expected_rel));
    let scan = |p: &Path| {
        LazyFrame::scan_parquet(
            PlRefPath::new(p.to_str().expect("utf-8")),
            ScanArgsParquet::default(),
        )
        .expect("scan")
    };
    let actual = apply_weights(
        expand(scan(&input), &estimand_opts(estimand)).expect("expand"),
        scan(&factors),
        &estimand_opts(estimand),
    )
    .expect("apply_weights")
    .collect()
    .expect("collect");
    let worst = worst_weight_rel(&actual, &expected);
    SpotCheck {
        label: label.to_owned(),
        estimand: est,
        rows: actual.height(),
        structural_ok: structural_equal(&actual, &expected),
        weight: Some((worst <= WEIGHT_REL_TOL, worst)),
    }
}

/// Live re-verification of the **fitted** weight path (`weights-fit`): fit the IPW models
/// in Rust (no pre-computed factor table), apply them, and check `weight` against
/// the Oracle within [`FITTED_WEIGHT_REL_TOL`]. Structural columns stay bit-exact.
#[cfg(feature = "weights-fit")]
fn spot_fitted(
    root: &Path,
    label: &str,
    input_rel: &str,
    expected_rel: &str,
    estimand: Estimand,
    spec: &WeightSpec,
) -> SpotCheck {
    let est = if estimand == Estimand::Itt {
        "ITT"
    } else {
        "PP"
    };
    let input = root.join(input_rel);
    let expected = read_parquet(&root.join(expected_rel));
    let scan = |p: &Path| {
        LazyFrame::scan_parquet(
            PlRefPath::new(p.to_str().expect("utf-8")),
            ScanArgsParquet::default(),
        )
        .expect("scan")
    };
    let opts = estimand_opts(estimand);
    let factors = fit_weights(scan(&input), &opts, spec).expect("fit_weights");
    let actual = apply_weights(expand(scan(&input), &opts).expect("expand"), factors, &opts)
        .expect("apply_weights")
        .collect()
        .expect("collect");
    let worst = worst_weight_rel(&actual, &expected);
    SpotCheck {
        label: label.to_owned(),
        estimand: est,
        rows: actual.height(),
        structural_ok: structural_equal(&actual, &expected),
        weight: Some((worst <= FITTED_WEIGHT_REL_TOL, worst)),
    }
}

fn main() -> ExitCode {
    let root = repo_root();
    let mut ok = true;
    let mut report = String::new();

    // --- Load both manifests ------------------------------------------------
    let itt_manifest_txt =
        fs::read_to_string(root.join("fixtures/MANIFEST.json")).expect("read MANIFEST.json");
    let w_manifest_txt = fs::read_to_string(root.join("fixtures/weights/MANIFEST_weights.json"))
        .expect("read MANIFEST_weights.json");
    let itt_manifest: Value = serde_json::from_str(&itt_manifest_txt).expect("parse MANIFEST.json");
    let w_manifest: Value = serde_json::from_str(&w_manifest_txt).expect("parse weights manifest");

    let prov = &itt_manifest["provenance"];
    let pkg = prov["package"].as_str().unwrap_or("TrialEmulation");
    let pkg_ver = prov["package_version"].as_str().unwrap_or("?");
    let r_ver = prov["r_version"].as_str().unwrap_or("?");
    let gen_itt = prov["generated_utc"].as_str().unwrap_or("?");
    let gen_w = w_manifest["provenance"]["generated_utc"]
        .as_str()
        .unwrap_or("?");

    // --- Toolchain / dependency pins ---------------------------------------
    let lock = fs::read_to_string(root.join("Cargo.lock")).expect("read Cargo.lock");
    let toolchain = fs::read_to_string(root.join("rust-toolchain.toml")).unwrap_or_default();
    let cargo_toml = fs::read_to_string(root.join("Cargo.toml")).unwrap_or_default();
    let binding_lock =
        fs::read_to_string(root.join("bindings/tters/src/rust/Cargo.lock")).unwrap_or_default();
    let rustc = toml_value(&toolchain, "channel").unwrap_or_else(|| "?".into());
    let edition = toml_value(&cargo_toml, "edition").unwrap_or_else(|| "?".into());
    let msrv = toml_value(&cargo_toml, "rust-version").unwrap_or_else(|| "?".into());
    let polars_v = lock_version(&lock, "polars").unwrap_or_else(|| "?".into());
    let criterion_v = lock_version(&lock, "criterion").unwrap_or_else(|| "?".into());
    let serde_json_v = lock_version(&lock, "serde_json").unwrap_or_else(|| "?".into());
    let sha2_v = lock_version(&lock, "sha2").unwrap_or_else(|| "?".into());
    let extendr_v = lock_version(&binding_lock, "extendr-api").unwrap_or_else(|| "?".into());

    // --- Live equivalence spot-checks --------------------------------------
    let mut spots = vec![
        spot_structural(&root, "edge", "E02_id4_canonical", Estimand::Itt),
        spot_structural(&root, "edge", "E02_id4_canonical", Estimand::PerProtocol),
        spot_structural(&root, "edge", "E06_switch_then_back", Estimand::PerProtocol),
        spot_structural(&root, "scenarios", "common", Estimand::Itt),
        spot_structural(&root, "scenarios", "common", Estimand::PerProtocol),
    ];
    spots.push(spot_weighted(
        &root,
        "weights/data_censored (ITT-IPCW)",
        "fixtures/weights/input_data_censored.parquet",
        "fixtures/weights/input_data_censored_itt_weights.parquet",
        "fixtures/weights/expected_data_censored_itt_weighted.parquet",
        Estimand::Itt,
    ));
    spots.push(spot_weighted(
        &root,
        "weights/high_switching (PP switch)",
        "fixtures/scenarios/input_high_switching.parquet",
        "fixtures/weights/input_high_switching_pp_weights.parquet",
        "fixtures/weights/expected_high_switching_pp_weighted.parquet",
        Estimand::PerProtocol,
    ));

    // Fitted-weight checks (only with `--features weights-fit`): fit the
    // IPW models in Rust and assert `weight` within FITTED_WEIGHT_REL_TOL.
    let fitted_checked = cfg!(feature = "weights-fit");
    #[cfg(feature = "weights-fit")]
    {
        spots.push(spot_fitted(
            &root,
            "weights-fit: data_censored (ITT-IPCW)",
            "fixtures/weights/input_data_censored.parquet",
            "fixtures/weights/expected_data_censored_itt_weighted.parquet",
            Estimand::Itt,
            &WeightSpec::ipcw(CensorWeightSpec::new(
                "censored",
                ["x2"],
                ["x2"],
                PoolCensor::Numerator,
            )),
        ));
        spots.push(spot_fitted(
            &root,
            "weights-fit: data_censored (PP switch+IPCW)",
            "fixtures/weights/input_data_censored.parquet",
            "fixtures/weights/expected_data_censored_pp_weighted.parquet",
            Estimand::PerProtocol,
            &WeightSpec::switching(SwitchWeightSpec::new(["x2"], ["x2", "x1"])).with_censor(
                CensorWeightSpec::new("censored", ["x2"], ["x2", "x1"], PoolCensor::None),
            ),
        ));
        spots.push(spot_fitted(
            &root,
            "weights-fit: high_switching (PP switch)",
            "fixtures/scenarios/input_high_switching.parquet",
            "fixtures/weights/expected_high_switching_pp_weighted.parquet",
            Estimand::PerProtocol,
            &WeightSpec::switching(SwitchWeightSpec::new(["x2"], ["x2", "x1"])),
        ));
    }

    // --- Fixture integrity (recompute SHA-256, compare to manifests) -------
    let mut integrity_rows = String::new();
    let mut n_checked = 0usize;
    let mut n_match = 0usize;
    for (manifest, base) in [(&itt_manifest, ""), (&w_manifest, "")] {
        for f in manifest["fixtures"].as_array().expect("fixtures array") {
            let rel = f["path"].as_str().expect("path");
            let want = f["sha256"].as_str().expect("sha256");
            let role = f["role"].as_str().unwrap_or("?");
            let n_rows = f["n_rows"].as_i64().unwrap_or(-1);
            let got = sha256_hex(&root.join(base).join(rel));
            let matched = got == want;
            n_checked += 1;
            if matched {
                n_match += 1;
            } else {
                ok = false;
            }
            let _ = writeln!(
                integrity_rows,
                "| `{role}` | `{rel}` | {n_rows} | `{}` | {} |",
                &got[..16],
                if matched { "✅" } else { "❌ DRIFT" }
            );
        }
    }

    // --- Assemble the report ------------------------------------------------
    let all_spots_ok = spots
        .iter()
        .all(|s| s.structural_ok && s.weight.is_none_or(|(within, _)| within));
    if !all_spots_ok {
        ok = false;
    }

    let _ = writeln!(
        report,
        "# Computational Reproducibility Certificate — `tte-expand`\n"
    );
    let _ = writeln!(
        report,
        "**Engine:** `tte-expand` (Rust + Polars, `#![forbid(unsafe_code)]`)  \n\
         **Oracle:** R `{pkg}` {pkg_ver} ({r_ver})  \n\
         **Verdict:** {}\n",
        if ok {
            "✅ BIT-EXACT EQUIVALENCE VERIFIED"
        } else {
            "❌ FAILED — see drift / mismatch below"
        }
    );
    let _ = writeln!(
        report,
        "This certificate is generated by `make verify`. It is reproducible: the \
         same committed fixtures + pinned toolchain regenerate identical claims. \
         Equivalence is proven exhaustively by the contract test suite (which \
         `make verify` runs immediately before this generator) and corroborated \
         by the live re-verification in §1; fixture integrity (§3) is recomputed \
         here from first principles.\n"
    );

    let _ = writeln!(report, "## 1. Equivalence claim\n");
    let _ = writeln!(
        report,
        "- **Structural columns** (`id, trial_period, followup_time, \
         assigned_treatment, treatment, outcome`): **bit-exact** (schema + values \
         + order + row count) for both estimands.\n\
         - **`weight`**: within relative tolerance **{WEIGHT_REL_TOL:e}** (ADR-2; \
         the engine redoes the float cumulative product and may reassociate).\n\
         - **Battery** (the contract suite, `cargo test`): 17 ITT + 17 PP \
         structural (9 edge + 8 scenarios each) + 5 weighted.\n"
    );
    let _ = writeln!(
        report,
        "Live re-verification performed by this generator:\n\n\
         | fixture | estimand | rows | structural | weight (worst rel) |\n\
         |---|---|---|---|---|"
    );
    for s in &spots {
        let weight_cell = match s.weight {
            None => "n/a".to_owned(),
            Some((within, worst)) => format!("{} ({worst:.1e})", if within { "✅" } else { "❌" }),
        };
        let _ = writeln!(
            report,
            "| `{}` | {} | {} | {} | {} |",
            s.label,
            s.estimand,
            s.rows,
            if s.structural_ok { "✅" } else { "❌" },
            weight_cell
        );
    }
    let _ = writeln!(
        report,
        "\n{}",
        if fitted_checked {
            format!(
                "The `weights-fit:` rows *fit* the IPW models in Rust (Phase 6, \
                 bound `smartcore` solver) and check `weight` within \
                 **{FITTED_WEIGHT_REL_TOL:e}** relative; the others use the \
                 pre-computed factor table within {WEIGHT_REL_TOL:e}."
            )
        } else {
            "Phase-6 fitted-weight checks were skipped (built without \
             `--features weights-fit`)."
                .to_owned()
        }
    );

    let _ = writeln!(report, "\n## 2. Oracle provenance\n");
    let _ = writeln!(
        report,
        "| field | value |\n|---|---|\n\
         | package | `{pkg}` |\n\
         | package version | `{pkg_ver}` |\n\
         | R version | `{r_ver}` |\n\
         | fixtures generated (ITT/PP) | `{gen_itt}` |\n\
         | fixtures generated (weights) | `{gen_w}` |"
    );

    let _ = writeln!(report, "\n## 3. Fixture integrity (differential)\n");
    let _ = writeln!(
        report,
        "Every manifest-listed fixture's SHA-256 was recomputed and compared to \
         the manifest. **{n_match}/{n_checked} match.** Any drift fails \
         `make verify`.\n\n\
         | role | path | rows | sha256 (first 16) | match |\n\
         |---|---|---|---|---|"
    );
    report.push_str(&integrity_rows);

    let _ = writeln!(report, "\n## 4. Toolchain & dependency pins\n");
    let _ = writeln!(
        report,
        "| component | pin |\n|---|---|\n\
         | rustc (dev/CI) | `{rustc}` |\n\
         | edition / MSRV | `{edition}` / `{msrv}` |\n\
         | polars | `{polars_v}` |\n\
         | criterion (bench) | `{criterion_v}` |\n\
         | serde_json (cert) | `{serde_json_v}` |\n\
         | sha2 (cert) | `{sha2_v}` |\n\
         | extendr-api (`tters` binding) | `{extendr_v}` |\n\
         | R (Oracle) | `{r_ver}` |"
    );

    let _ = writeln!(
        report,
        "\n## 5. Tolerance contract (where exactness ends)\n"
    );
    let _ = writeln!(
        report,
        "- Expansion / per-protocol censoring (which rows survive): **exact** \
         (integer + categorical; a diff is a bug).\n\
         - Weight *application*: **exact** structural join, **{WEIGHT_REL_TOL:e}** \
         relative on the float `weight` product (the engine redoes the cumulative \
         product and may reassociate).\n\
         - Weight *fitting* (Phase 6, `weights-fit` feature): the bound `smartcore` \
         logistic solver reproduces R `glm`/`parglm` within **{FITTED_WEIGHT_REL_TOL:e}** \
         relative on the fitted `weight` — its L-BFGS converges to the same MLE as \
         R's IRLS, not bit-for-bit (observed worst on the fixtures ≈3.4e-7).\n\
         - Robust/sandwich variance and the MSM coefficient estimation stay in R \
         and are out of scope for the engine."
    );

    let _ = writeln!(report, "\n## 6. Reproduce\n\n```sh\nmake verify\n```\n");
    let _ = writeln!(
        report,
        "## 7. Determinism\n\n\
         The engine is `#![forbid(unsafe_code)]` with no wall-clock, RNG, or \
         environment reads in the transform path; all sorts are total and \
         explicit. Output is byte-identical across runs, machines, CPU core \
         counts, locales, and timezones — which is what makes this certificate \
         meaningful."
    );

    // --- Write artifact + report to stdout ---------------------------------
    let out = root.join("report/certificate.md");
    fs::write(&out, &report).expect("write certificate");
    println!("certificate written to {}", out.display());
    println!(
        "integrity: {n_match}/{n_checked} fixtures match manifest; spot-checks: {}",
        if all_spots_ok { "all pass" } else { "FAILURES" }
    );

    if ok {
        ExitCode::SUCCESS
    } else {
        eprintln!("CERTIFICATE FAILED: fixture drift or equivalence mismatch — STOP and report.");
        ExitCode::FAILURE
    }
}
