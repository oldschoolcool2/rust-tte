#!/usr/bin/env bash
# Hook: PreToolUse → Bash, Edit, Write
export PATH="/usr/local/bin:/usr/bin:/bin:$PATH"
# Prevents modification of Claude hooks and settings without explicit approval.
set -eo pipefail

INPUT=$(cat) || exit 0
TOOL=$(echo "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null) || exit 0

PROTECTED_PATHS=(
    ".claude/hooks/"
    ".claude/settings.json"
)

deny_with_reason() {
    local reason="$1"
    jq -n \
        --arg name "PreToolUse" \
        --arg reason "$reason" \
        '{hookSpecificOutput: {hookEventName: $name, permissionDecision: "deny", permissionDecisionReason: $reason}}'
    exit 0
}

check_path() {
    local path="$1"
    for protected in "${PROTECTED_PATHS[@]}"; do
        if [[ $path == *"$protected"* ]]; then
            deny_with_reason "Protected path '$protected' cannot be modified without explicit user permission. Ask the user before changing Claude hooks or settings."
        fi
    done
}

case "$TOOL" in
Bash)
    COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0
    for protected in "${PROTECTED_PATHS[@]}"; do
        if [[ $COMMAND == *"$protected"* ]]; then
            if [[ $COMMAND == *"rm "*"$protected"* ]] ||
                [[ $COMMAND == *"mv "*"$protected"* ]] ||
                [[ $COMMAND == *">"*"$protected"* ]] ||
                [[ $COMMAND == *">>"*"$protected"* ]] ||
                [[ $COMMAND == *"sed "*"$protected"* ]] ||
                [[ $COMMAND == *"chmod "*"$protected"* ]] ||
                [[ $COMMAND == *"cp "*"$protected"* ]]; then
                deny_with_reason "Protected path '$protected' cannot be modified via shell redirection or destructive commands. Ask the user for explicit permission before modifying Claude hooks or settings."
            fi
        fi
    done
    ;;
Edit | Write)
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null) || exit 0
    check_path "$FILE_PATH"
    ;;
esac

exit 0
