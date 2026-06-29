#!/usr/bin/env bash
# Hook: PostToolUse → Write, Edit
export PATH="$HOME/.cargo/bin:/usr/local/bin:/usr/bin:/bin:$PATH"
# Auto-formats Rust files after the agent edits them, so generated code always
# matches `cargo fmt` / CI. Degrades silently when the Rust toolchain is absent
# (e.g. cargo not installed) — never blocks the edit.
set -eo pipefail

INPUT=$(cat) || exit 0
FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null) || exit 0

if [ -z "$FILE_PATH" ]; then
    exit 0
fi

# Only format Rust source.
if [[ $FILE_PATH != *.rs ]]; then
    exit 0
fi

# `rustfmt` formats a single file directly and respects rustfmt.toml found by
# walking up from the file (which pins edition = 2024). Prefer it over
# `cargo fmt` (whole package) to keep the hook fast and scoped to the edited file.
if command -v rustfmt &>/dev/null; then
    rustfmt "$FILE_PATH" 2>/dev/null || true
fi

exit 0
