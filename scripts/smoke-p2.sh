#!/usr/bin/env bash
# P2 end-to-end smoke test against live Postgres + Rust backend.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BACKEND_DIR="$ROOT/apps/backend"
SERVER_PID=""

cleanup() {
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

log() { echo "[smoke-p2] $*"; }
fail() { echo "[smoke-p2] FAIL: $*" >&2; exit 1; }

require_cmd() {
  command -v "$1" >/dev/null 2>&1 || fail "missing required command: $1"
}

require_cmd curl
require_cmd jq

if [[ -f "$BACKEND_DIR/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$BACKEND_DIR/.env"
  set +a
fi

BASE_URL="${SMOKE_BASE_URL:-http://localhost:${PORT:-3000}}"
START_SERVER="${SMOKE_START_SERVER:-1}"
ADMIN_USER="${SMOKE_ADMIN_USERNAME:-}"
ADMIN_PASS="${SMOKE_ADMIN_PASSWORD:-}"

if [[ -z "$ADMIN_USER" || -z "$ADMIN_PASS" ]]; then
  fail "Set SMOKE_ADMIN_USERNAME and SMOKE_ADMIN_PASSWORD for admin login smoke test"
fi

if [[ "$START_SERVER" == "1" ]]; then
  log "Starting backend on $BASE_URL ..."
  (cd "$BACKEND_DIR" && cargo run -q) &
  SERVER_PID=$!
  for _ in $(seq 1 60); do
    if curl -sf "$BASE_URL/health/live" >/dev/null 2>&1; then
      break
    fi
    sleep 1
  done
fi

log "1/8 GET /health/live"
HEALTH=$(curl -sf "$BASE_URL/health/live")
echo "$HEALTH" | jq -e '.success == true and .data.status == "ok"' >/dev/null \
  || fail "health/live envelope unexpected: $HEALTH"
DB_STATUS=$(echo "$HEALTH" | jq -r '.data.db // empty')
if [[ "$DB_STATUS" != "up" ]]; then
  fail "database not up (got: ${DB_STATUS:-missing})"
fi

log "2/8 POST /auth/login/admin"
LOGIN=$(curl -sf -X POST "$BASE_URL/auth/login/admin" \
  -H 'Content-Type: application/json' \
  -d "{\"username\":\"$ADMIN_USER\",\"password\":\"$ADMIN_PASS\"}")
TOKEN=$(echo "$LOGIN" | jq -r '.data.accessToken // empty')
[[ -n "$TOKEN" ]] || fail "login/admin did not return accessToken: $LOGIN"
AUTH_HEADER="Authorization: Bearer $TOKEN"

auth_get() {
  local path="$1"
  curl -sf "$BASE_URL$path" -H "$AUTH_HEADER"
}

auth_post_json() {
  local path="$1"
  local body="$2"
  curl -sf -X POST "$BASE_URL$path" \
    -H "$AUTH_HEADER" \
    -H 'Content-Type: application/json' \
    -d "$body"
}

log "3/8 Authenticated reads: stats, devices, games, plans, player-plans"
for path in /stats/dashboard /devices /games /plans /player-plans; do
  RESP=$(auth_get "$path")
  echo "$RESP" | jq -e '.success == true' >/dev/null \
    || fail "GET $path failed envelope: $RESP"
done

log "4/8 SSE endpoint accepts auth"
SSE_CODE=$(curl -s -o /dev/null -w '%{http_code}' -N --max-time 2 \
  "$BASE_URL/sse?topics=session,device" -H "$AUTH_HEADER" || true)
[[ "$SSE_CODE" == "200" ]] || fail "SSE endpoint expected 200, got $SSE_CODE"

log "5/8 POST /player-plans (assign plan to player)"
if ! command -v psql >/dev/null 2>&1; then
  fail "psql required for player-plans smoke steps (or set SMOKE_PLAN_ID and SMOKE_PLAYER_ID)"
fi
DB_URL="${DATABASE_URL:-}"
if [[ -z "$DB_URL" ]]; then
  DB_HOST="${DB_HOST:-localhost}"
  DB_PORT="${DB_PORT:-5432}"
  DB_USERNAME="${DB_USERNAME:-postgres}"
  DB_PASSWORD="${DB_PASSWORD:-postgres}"
  DB_DATABASE="${DB_DATABASE:-gamezone_dev}"
  DB_URL="postgres://${DB_USERNAME}:${DB_PASSWORD}@${DB_HOST}:${DB_PORT}/${DB_DATABASE}"
fi
PLAN_ID="${SMOKE_PLAN_ID:-$(psql "$DB_URL" -t -A -c \
  "SELECT id FROM plans WHERE \"isActive\" = true AND \"deletedAt\" IS NULL ORDER BY \"createdAt\" DESC LIMIT 1;" | tr -d '[:space:]')}"
PLAYER_ID="${SMOKE_PLAYER_ID:-$(psql "$DB_URL" -t -A -c \
  "SELECT id FROM users WHERE role = 'player' AND \"isActive\" = true AND \"deletedAt\" IS NULL ORDER BY \"createdAt\" DESC LIMIT 1;" | tr -d '[:space:]')}"
[[ -n "$PLAN_ID" ]] || fail "no active plan found in database (set SMOKE_PLAN_ID)"
[[ -n "$PLAYER_ID" ]] || fail "no active player found in database (set SMOKE_PLAYER_ID)"

ASSIGN=$(auth_post_json "/player-plans" \
  "{\"playerId\":\"$PLAYER_ID\",\"planId\":\"$PLAN_ID\"}")
echo "$ASSIGN" | jq -e '.success == true and (.data.id | length) > 0' >/dev/null \
  || fail "POST /player-plans failed envelope: $ASSIGN"
PP_ID=$(echo "$ASSIGN" | jq -r '.data.id')

log "6/8 POST /player-plans/{id}/validate"
VALIDATE=$(auth_post_json "/player-plans/$PP_ID/validate" "{}")
echo "$VALIDATE" | jq -e '.success == true and .data.valid == true' >/dev/null \
  || fail "POST /player-plans/$PP_ID/validate failed: $VALIDATE"

log "7/8 GET /player-plans/{id}"
GET_PP=$(auth_get "/player-plans/$PP_ID")
echo "$GET_PP" | jq -e '.success == true and .data.id == "'"$PP_ID"'"' >/dev/null \
  || fail "GET /player-plans/$PP_ID failed: $GET_PP"

log "All smoke checks passed."
