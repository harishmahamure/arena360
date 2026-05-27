#!/usr/bin/env bash
# Logs architecture-sensitive file edits to docs/adr/.events.jsonl.
# Observe-only — always exits 0, never blocks edits.
set -euo pipefail

INPUT=$(cat)
EVENTS_FILE="docs/adr/.events.jsonl"

# Extract fields from hook stdin JSON (python3 for portability)
read -r FILE_PATH TOOL_NAME <<< "$(echo "$INPUT" | python3 -c "
import json, sys
try:
    d = json.load(sys.stdin)
    fp = d.get('file_path') or d.get('path') or ''
    tn = d.get('tool_name') or d.get('tool') or 'unknown'
    print(fp, tn)
except Exception:
    print('', 'unknown')
")"

[[ -z "$FILE_PATH" ]] && exit 0

# Root package.json only — skip apps/*/package.json and packages/*/package.json
if [[ "$FILE_PATH" == *"/package.json" && "$FILE_PATH" != "package.json" && "$FILE_PATH" != "./package.json" ]]; then
  exit 0
fi

# Architecture-sensitive path patterns (* matches any string including /)
case "$FILE_PATH" in
  apps/backend/src/*|apps/backend/migrations/*) ;;
  apps/backend/Cargo.toml|apps/backend/Cargo.lock) ;;
  infra/*) ;;
  packages/contracts/*|packages/api-types/*) ;;
  docs/adr/*) ;;
  turbo.json|pnpm-workspace.yaml|package.json|./package.json) ;;
  *) exit 0 ;;
esac

mkdir -p "$(dirname "$EVENTS_FILE")"
TS=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

python3 -c "
import json
print(json.dumps({
    'ts': '$TS',
    'source': 'hook',
    'kind': 'edit',
    'tool': $(python3 -c "import json; print(json.dumps('$TOOL_NAME'))"),
    'path': $(python3 -c "import json; print(json.dumps('$FILE_PATH'))")
}))
" >> "$EVENTS_FILE"

exit 0
