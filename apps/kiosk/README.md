# Arena360 Windows kiosk

The kiosk is the locked-down Windows station client described in
[the current system documentation](../../docs/SYSTEM.md). It handles device
provisioning, player authentication and registration, timed sessions, the local
software allow-list, process launch and cleanup, ordering, realtime updates,
offline grace, and idle-only auto-update.

## Develop and test

```bash
cp apps/kiosk/.env.example apps/kiosk/.env
pnpm kiosk:dev
pnpm --filter @gaming-cafe/kiosk test
pnpm --filter @gaming-cafe/kiosk test:rust
```

Build the Windows NSIS installer on Windows:

```bash
pnpm --filter @gaming-cafe/kiosk tauri:build
```

Configuration, release behavior, diagnostics, and security boundaries are in
[deployment and operations](../../docs/DEPLOYMENT.md).
