# Git Workflow

- **Default branch:** `main`. Never commit directly to it — use a branch.
- **Branches:** `feat/<name>`, `fix/<name>`, `chore/<name>`, `docs/<name>`.
- **Conventional commits:** `feat:`, `fix:`, `chore:`, `docs:`, `refactor:`, `test:`,
  `perf:`, `ci:`, `build:`. One logical change per commit (atomic).
- **Before committing:** run `pre-commit run --all-files` (or let the installed
  git hook run) — it runs gitleaks, fmt/clippy, and the lint suite. Fix, don't
  bypass. Never use `--no-verify`.
- **Moving files:** always `git mv` (not `mv`) for tracked files, to preserve
  history. Untracked scratch belongs under `tmp/`.
- **Never force-push** to `main`. Use `--force-with-lease` on your own branch only,
  with explicit user confirmation.
- **Never** run history-destroying commands (`reset --hard`, `clean -fd`,
  `checkout .`, `stash drop/clear`) without explicit user confirmation — the
  git-safety hook blocks them by default.
- **Commits are made only when the user asks.** If on `main`/`master`, branch first.
