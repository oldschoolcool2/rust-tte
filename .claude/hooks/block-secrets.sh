#!/usr/bin/env bash
# Hook: PreToolUse → Bash, Edit, Write
export PATH="/usr/local/bin:/usr/bin:/bin:$PATH"
# Defense-in-depth against secret leakage: blocks creating credential files and
# blocks writing obvious private-key material. This complements (does not
# replace) the gitleaks pre-commit hook and CI scan, which guard the commit.
set -eo pipefail

INPUT=$(cat) || exit 0
TOOL=$(echo "$INPUT" | jq -r '.tool_name // empty' 2>/dev/null) || exit 0

deny_with_reason() {
    local reason="$1"
    jq -n \
        --arg name "PreToolUse" \
        --arg reason "$reason" \
        '{hookSpecificOutput: {hookEventName: $name, permissionDecision: "deny", permissionDecisionReason: $reason}}'
    exit 0
}

# True for credential-bearing file paths. `.env.example` / `.env.sample` /
# `.env.template` are allowed (they are committed onboarding stubs).
is_secret_path() {
    local p="$1"
    local base="${p##*/}"
    case "$base" in
    .env.example | .env.sample | .env.template | *.example | *.sample) return 1 ;;
    esac
    case "$p" in
    *.pem | *.key | *.p12 | *.pfx | *.keystore | *.jks | *id_rsa | *id_ed25519) return 0 ;;
    esac
    case "$base" in
    .env | .env.* | .npmrc | .pypirc | credentials | .netrc) return 0 ;;
    esac
    return 1
}

case "$TOOL" in
Edit | Write)
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty' 2>/dev/null) || exit 0
    CONTENT=$(echo "$INPUT" | jq -r '.tool_input.new_string // .tool_input.content // empty' 2>/dev/null) || exit 0
    if [ -n "$FILE_PATH" ] && is_secret_path "$FILE_PATH"; then
        deny_with_reason "Refusing to write credential file '$FILE_PATH'. Secrets must never be committed — use .env.example with placeholders, or store real secrets outside the repo. If this is a legitimate non-secret, ask the user."
    fi
    if [[ $CONTENT == *"-----BEGIN "*"PRIVATE KEY-----"* ]]; then
        deny_with_reason "Refusing to write a private key block into '$FILE_PATH'. Private keys must never live in the repo. Load them from the environment at runtime."
    fi
    ;;
Bash)
    COMMAND=$(echo "$INPUT" | jq -r '.tool_input.command // empty' 2>/dev/null) || exit 0
    [ -z "$COMMAND" ] && exit 0
    # Block shell redirection that creates an .env or key file.
    if [[ $COMMAND =~ \>\>?[[:space:]]*\.?[A-Za-z0-9_/.-]*\.env([[:space:]]|$) ]] ||
        [[ $COMMAND =~ \>\>?[[:space:]]*[A-Za-z0-9_/.-]*\.(pem|key|p12|pfx)([[:space:]]|$) ]]; then
        deny_with_reason "Refusing shell redirection into a credential file (.env / *.pem / *.key). Secrets must never be committed; write a .env.example placeholder instead."
    fi
    ;;
esac

exit 0
