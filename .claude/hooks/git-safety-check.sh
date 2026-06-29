#!/usr/bin/env bash
# Hook: PreToolUse → Bash
export PATH="/usr/local/bin:/usr/bin:/bin:$PATH"
# Blocks destructive git commands that could lose work.
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

# Substring checks (order preserved where it matters).
if [[ $COMMAND == *"git checkout -- "* ]]; then
    deny_with_reason "Destructive: 'git checkout --' discards uncommitted changes. Use 'git stash' to save work first, or ask the user for explicit confirmation."
fi
if [[ $COMMAND == *"git checkout ."* ]]; then
    deny_with_reason "Destructive: 'git checkout .' discards uncommitted changes in the working tree. Use 'git stash' or ask the user for explicit confirmation."
fi
if [[ $COMMAND == *"git restore -- "* ]]; then
    deny_with_reason "Destructive: 'git restore --' discards uncommitted changes. Use 'git stash' or ask the user for explicit confirmation."
fi
if [[ $COMMAND == *"git restore ."* ]]; then
    deny_with_reason "Destructive: 'git restore .' discards uncommitted changes. Use 'git stash' or ask the user for explicit confirmation."
fi
if [[ $COMMAND == *"git reset --hard"* ]]; then
    deny_with_reason "Destructive: 'git reset --hard' permanently discards commits and working tree changes. Use 'git stash', 'git reset --soft', or ask the user for confirmation."
fi
if [[ $COMMAND == *"git clean -f"* ]] || [[ $COMMAND == *"git clean -fd"* ]] || [[ $COMMAND == *"git clean -fx"* ]]; then
    deny_with_reason "Destructive: 'git clean' permanently deletes untracked files (fixtures, oracle output, local scratch). Preview with 'git clean -n', or ask the user for explicit confirmation."
fi
if [[ $COMMAND == *"git stash drop"* ]]; then
    deny_with_reason "Destructive: 'git stash drop' permanently deletes a stash entry. Ask the user for explicit confirmation before dropping stashes."
fi
if [[ $COMMAND == *"git stash clear"* ]]; then
    deny_with_reason "Destructive: 'git stash clear' permanently deletes ALL stash entries. Ask the user for explicit confirmation."
fi
if [[ $COMMAND == *"git revert"* ]]; then
    deny_with_reason "Potentially destructive: 'git revert' creates commits that undo history. Ask the user for explicit confirmation before reverting commits."
fi
if [[ $COMMAND == *"git push --force"* ]] || [[ $COMMAND == *"git push -f "* ]]; then
    deny_with_reason "Destructive: force-push overwrites remote history and can destroy teammates' work. Use 'git push --force-with-lease' if appropriate, or ask the user for explicit confirmation."
fi

exit 0
