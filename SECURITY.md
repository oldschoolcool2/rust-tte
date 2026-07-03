# Security Policy

## Supported versions

`tters` / `tte-expand` is pre-1.0; fixes land on the latest released version.
Please test against the newest
[r-universe build](https://oldschoolcool2.r-universe.dev/tters) or `main` before
reporting.

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |
| < 0.1   | :x:                |

## Reporting a vulnerability

Please report suspected vulnerabilities **privately** — do not open a public
issue for a security report.

- **Preferred:** GitHub
  [private vulnerability reporting](https://github.com/oldschoolcool2/rust-tte/security/advisories/new)
  (repository **Security → Report a vulnerability**).
- **Alternatively:** email <oldschoolcool@gmail.com> with details and, if
  possible, a minimal reproduction.

Expect an acknowledgement within a few days. Once a fix is available we will
coordinate disclosure and credit reporters who wish to be named.

## Threat model

This project is a deterministic, offline **data-transformation library**, not a
networked service: it reads local Parquet / `data.frame` inputs and produces
expanded outputs. The realistic risk surface is therefore the **software supply
chain** rather than runtime exploitation:

- Dependencies are gated by [`cargo deny`](deny.toml) (advisories, licenses,
  bans, sources), and `Cargo.lock` is committed for reproducible builds.
- Secrets are kept out of the tree by `gitleaks` (pre-commit + CI) and a
  block-secrets guard.
- The engine crate is `#![forbid(unsafe_code)]`.

Memory-safety, determinism, and supply-chain issues are all in scope — if you
find one, we want to know.
