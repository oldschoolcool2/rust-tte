# Security & Secret Hygiene

This repo is data-processing, not a networked service — the main risk is leaking
secrets into git and pulling in compromised dependencies.

## Secrets
- **Never commit** `.env`, API tokens, private keys (`*.pem`, `*.key`, `id_rsa`),
  keystores, or credentials of any kind. The block-secrets hook refuses to write
  them; gitleaks (pre-commit + CI) refuses to commit them.
- Commit **`.env.example`** with placeholder values for onboarding — never the
  real file.
- Load any real secret from the environment at runtime; never hard-code it.
- If gitleaks flags a genuine false positive, add a narrow allowlist entry to
  `.gitleaks.toml` (a specific regex/path) — never disable the scan.

## Supply chain
- New dependencies must pass `cargo deny check` (advisories, licenses, bans,
  sources). Justify every new dependency (CLAUDE.md) and prefer std / crates
  already in the tree.
- Keep `Cargo.lock` committed for reproducible builds. Dependabot proposes
  updates; review advisory bumps promptly.
- The crate is `#![forbid(unsafe_code)]` — no `unsafe`, ever.

## CI / GitHub Actions
- Every `uses:` is pinned to a 40-char commit SHA with a trailing `# vX.Y.Z`
  comment; `zizmor` enforces this and other supply-chain audits.
- Workflows declare least-privilege `permissions:` (read-only by default).
- Never echo a secret into logs or interpolate `${{ secrets.* }}` directly into a
  `run:` block — pass via `env:` (template-injection guard).
