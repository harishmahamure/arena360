#!/usr/bin/env bash
set -euo pipefail

bash scripts/check-vision.sh

STAGED="$(git diff --cached --name-only --diff-filter=ACMR)"
if [ -z "$STAGED" ]; then
  exit 0
fi

pnpm exec biome check --write --staged --no-errors-on-unmatched

echo "$STAGED" | xargs git add
