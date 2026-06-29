---
description: Scan the working tree and git history for leaked secrets with gitleaks (same ruleset as CI).
allowed-tools: Bash(gitleaks *), Bash(pre-commit run *), Bash(git status*), Bash(git log*), Read
---

# /check-secrets

Run the same secret-leakage scan locally that CI enforces, using the repo's
`.gitleaks.toml` allowlist.

## Steps

1. Confirm gitleaks is available: `gitleaks version`. If missing, tell the user to
   install it (`brew install gitleaks`, or download the pinned `8.30.1` release —
   the version CI uses) and stop.
2. **Staged / working tree** (what a commit would introduce):
   `gitleaks git --staged --redact --verbose` — if nothing is staged, run
   `gitleaks dir . --redact --verbose` to scan the current files.
3. **Full history** (what is already committed):
   `gitleaks git . --redact --verbose`
4. Report results:
   - **Clean:** state that no secrets were found and which scopes were scanned.
   - **Findings:** for each, give the file, rule, and commit (values are redacted
     by `--redact` — never print the raw secret). Then advise:
     - Remove the secret from the file; load it from the environment instead.
     - If it is a false positive, add a **narrow** entry (specific regex or path)
       to `.gitleaks.toml` — never disable the scan wholesale.
     - If it was already committed, warn that history rewriting + secret rotation
       is required, and that this is a human decision (do not rewrite history
       automatically).

## Notes

- This mirrors `.github/workflows/secret-scan.yml` and the gitleaks pre-commit
  hook; a clean result here means the CI scan will pass.
- Never echo unredacted secret material into the chat or logs.
