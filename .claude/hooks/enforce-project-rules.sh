#!/usr/bin/env bash
# Hook: PreToolUse → Bash
export PATH="/usr/local/bin:/usr/bin:/bin:$PATH"
# Enforces project-specific rules for shell commands.
set -eo pipefail

INPUT=$(cat) || exit 0
COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0

if [ -z "$COMMAND" ]; then
    exit 0
fi

deny_with_reason() {
    local reason="$1"
    jq -n \
        --arg name "PreToolUse" \
        --arg reason "$reason" \
        '{hookSpecificOutput: {hookEventName: $name, permissionDecision: "deny", permissionDecisionReason: $reason}}'
    exit 0
}

# Block git status -uall (can cause memory issues on large repos).
if [[ $COMMAND == *"git status"*"-uall"* ]]; then
    deny_with_reason "Do not use 'git status -uall' — it can cause memory issues on large repos. Use 'git status' without -uall."
fi

# Block git config writes (should not modify the user's git config).
if [[ $COMMAND == *"git config"* ]] && [[ $COMMAND != *"--get"* ]] && [[ $COMMAND != *"--list"* ]]; then
    deny_with_reason "Do not modify git configuration. Only 'git config --get' and 'git config --list' are allowed."
fi

# Prefer `git mv` over `mv` for tracked files (CLAUDE.md convention). Only nudge
# when moving paths that look tracked; never hard-block (mv on untracked scratch
# is legitimate).
if [[ $COMMAND == *"mv "* ]] && [[ $COMMAND != *"git mv"* ]] && [[ $COMMAND == *"crates/"* || $COMMAND == *"docs/"* || $COMMAND == *"oracle/"* ]]; then
    deny_with_reason "Use 'git mv' (not 'mv') to relocate tracked files so history is preserved (see CLAUDE.md). If the file is untracked scratch, move it under tmp/ instead."
fi

exit 0
