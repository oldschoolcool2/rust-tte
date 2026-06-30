# Tier-2 Whole-Pipeline Golden — Rust-expand -> R-estimate

**Verdict:** PASS

The Rust engine expands `data_censored` (ITT); R's `trial_msm()` estimates on
that frame; the coefficients are compared to upstream `initiators()`. No glm /
sandwich code runs in Rust. Tolerance: log-OR 1e-4, robust SE 1e-3.

- max |Δ estimate| = 1.978e-10 (tol 1e-04)
- max |Δ robust_se| = 1.773e-04 (tol 1e-03)

| term | est (Rust→R) | est (upstream) | Δest | rse (Rust→R) | rse (upstream) | Δrse |
|---|---:|---:|---:|---:|---:|---:|
| `(Intercept)` | -5.90101789 | -5.90101789 | 2.04e-14 | 0.75559365 | 0.75559365 | 3.06e-14 |
| `assigned_treatment` | 1.41503855 | 1.41503855 | 6.66e-16 | 0.50624118 | 0.50624118 | 2.44e-15 |
| `followup_time` | 0.33848377 | 0.33848377 | 1.61e-15 | 0.23706284 | 0.23706284 | 8.19e-15 |
| `I(followup_time^2)` | -0.02076696 | -0.02076696 | 3.12e-17 | 0.01395121 | 0.01395121 | 3.76e-16 |
| `I(trial_period^2)` | -7.50057759 | -7.50057759 | 1.98e-10 | 0.55271713 | 0.55289441 | 1.77e-04 |
| `trial_period` | 6.99418491 | 6.99418491 | 1.98e-10 | 0.99407168 | 0.99417026 | 9.86e-05 |
