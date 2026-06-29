#!/usr/bin/env bash
# Hook: PreToolUse → Bash, Edit, Write
export PATH="/usr/local/bin:/usr/bin:/bin:$PATH"
# Enforces CLAUDE.md's "You MUST NEVER edit" contract: the Oracle, the immutable
# Parquet fixtures, the test harness, and the spec are ground truth. Reads are
# always allowed — this only blocks writes / destructive shell ops.
set -eo pipefail

INPUT=$(cat) || exit 0
TOOL=$(echo "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null) || exit 0

# Path fragments that are ground truth and must not be mutated by the agent.
# Matched as substrings of the absolute or repo-relative path.
PROTECTED_PATHS=(
    "fixtures/"
    "oracle/"
    "/tests/"
    "SPEC.md"
)

deny_with_reason() {
    local reason="$1"
    jq -n \
        --arg name "PreToolUse" \
        --arg reason "$reason" \
        '{hookSpecificOutput: {hookEventName: $name, permissionDecision: "deny", permissionDecisionReason: $reason}}'
    exit 0
}

reason_for() {
    local p="$1"
    case "$p" in
    "fixtures/") echo "fixtures/ holds the immutable Parquet ground truth. If a fixture seems wrong, STOP and report the offending rows — do not edit it." ;;
    "oracle/") echo "oracle/ is the R reference implementation that generates ground truth. The agent never edits it (see CLAUDE.md)." ;;
    "/tests/") echo "The test harness (tests/) defines the contract and tolerances. Do not edit tests, add #[ignore], or weaken assertions to force a pass." ;;
    "SPEC.md") echo "SPEC.md is orientation only and is owned by the human. If SPEC.md and a fixture disagree, the FIXTURE WINS — report the discrepancy." ;;
    *) echo "This path is ground truth and must not be modified by the agent." ;;
    esac
}

case "$TOOL" in
Edit | Write | NotebookEdit)
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // .tool_input.notebook_path // empty' 2>/dev/null) || exit 0
    [ -z "$FILE_PATH" ] && exit 0
    for protected in "${PROTECTED_PATHS[@]}"; do
        if [[ $FILE_PATH == *"$protected"* ]]; then
            deny_with_reason "BLOCKED edit to a ground-truth path ('$protected'). $(reason_for "$protected")"
        fi
    done
    ;;
Bash)
    COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0
    [ -z "$COMMAND" ] && exit 0
    for protected in "${PROTECTED_PATHS[@]}"; do
        if [[ $COMMAND == *"$protected"* ]]; then
            # Only block writes / destructive ops; reading and running tests is fine.
            if [[ $COMMAND == *"rm "*"$protected"* ]] ||
                [[ $COMMAND == *"mv "*"$protected"* ]] ||
                [[ $COMMAND == *"cp "*"$protected"* ]] ||
                [[ $COMMAND == *">"*"$protected"* ]] ||
                [[ $COMMAND == *">>"*"$protected"* ]] ||
                [[ $COMMAND == *"truncate "*"$protected"* ]] ||
                [[ $COMMAND == *"tee "*"$protected"* ]] ||
                { [[ $COMMAND == *"sed "* ]] && [[ $COMMAND == *"-i"* ]] && [[ $COMMAND == *"$protected"* ]]; }; then
                deny_with_reason "BLOCKED shell write to a ground-truth path ('$protected'). $(reason_for "$protected")"
            fi
        fi
    done
    ;;
esac

exit 0
