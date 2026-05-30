# @gaming-cafe/kiosk

Windows Tauri 2 + React 19 in-cafe kiosk. See [REQUIREMENTS-KIOSK.md](../../docs/REQUIREMENTS-KIOSK.md) and [PLANNER-KIOSK.md](../../docs/PLANNER-KIOSK.md).

## Environment

Copy the example env file and point it at your backend (default Axum port is often `3001` locally):

```bash
cp apps/kiosk/.env.example apps/kiosk/.env
# edit VITE_API_URL if needed
```

| Variable | Purpose |
|----------|---------|
| `VITE_API_URL` | REST API base URL (WebSocket uses `/realtime` on the same host) |

## Dev

From repo root:

```bash
pnpm install
cp apps/kiosk/.env.example apps/kiosk/.env   # first time only
pnpm kiosk:dev
```

Or:

```bash
pnpm --filter @gaming-cafe/kiosk tauri:dev
```

Rust tests:

```bash
pnpm --filter @gaming-cafe/kiosk test:rust
```
