# @gaming-cafe/kiosk

Windows Tauri 2 + React 19 in-cafe kiosk. See [REQUIREMENTS-KIOSK.md](../../docs/REQUIREMENTS-KIOSK.md) and [PLANNER-KIOSK.md](../../docs/PLANNER-KIOSK.md).

## Dev

From repo root:

```bash
pnpm install
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
