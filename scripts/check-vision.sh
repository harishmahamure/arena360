#!/usr/bin/env bash
set -euo pipefail

VISION_STAGED=false
REVIEW_NEEDED=false

while IFS= read -r -d '' path; do
  case "$path" in
    VISION.md) VISION_STAGED=true ;;
    apps/*|packages/*|infra/*|.github/workflows/*|docs/architecture/*|docs/adr/*|docs/plans/*|package.json|pnpm-workspace.yaml|turbo.json|docker-compose.yml)
      REVIEW_NEEDED=true
      ;;
  esac
done < <(git diff --cached --name-only --no-renames -z)

if "$REVIEW_NEEDED" && ! "$VISION_STAGED"; then
  cat >&2 <<'MESSAGE'
[vision] Review VISION.md for the staged changes. Update affected capabilities,
architecture, limitations, or readiness in the same commit when their meaning
changes. If the vision remains accurate, no document edit is needed.
[vision] Reminder only; this check does not block the commit.
MESSAGE
fi
