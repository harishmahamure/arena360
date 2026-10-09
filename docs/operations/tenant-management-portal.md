# Tenant management portal

The internal portal at `/portal.html` manages platform operators, plans, tenants, cells and tenant administrators. Operator accounts are separate from tenant business accounts.

## Start locally

Configure the backend with `CONTROL_DATABASE_URL`, `ARENA_CELL_ID`, `TENANT_DATA_DIR`, `JWT_SECRET` and `PORT`, then run `pnpm backend:dev`. The control migration creates the platform operator and plan tables. `PLATFORM_ADMIN_TOKEN` is no longer used.

Create the first operator with a password sent through stdin:

```sh
read -rs 'OPERATOR_PASSWORD?Platform password: '; printf '\n'
printf '%s\n' "$OPERATOR_PASSWORD" | CONTROL_DATABASE_URL=postgres://USER:PASSWORD@127.0.0.1:5432/arena_portal cargo run --manifest-path apps/backend/Cargo.toml --bin platform_operator -- operator
unset OPERATOR_PASSWORD
```

The password must be 12–72 bytes. Run `pnpm portal:dev` and open `http://localhost:5174/portal.html`. Sign in with the operator username and password. First sign-in presents a QR code and manual key; add it to an authenticator app and verify a six-digit code before any management endpoint is accessible. Later sign-ins require a fresh code. Sessions last 12 hours, stay in browser memory and can be revoked by signing out. Five failed password attempts lock that account for 15 minutes; each MFA challenge allows five code attempts and expires after 10 minutes.

Use **Cells** to register the backend's exact `ARENA_CELL_ID` and origin. Use **Tenants** to provision a tenant and its first business administrator. Use **Plans** to create or edit a reusable code, entitlements and grace period. The trial plan exists by default. In a tenant workspace, assign an active plan and future expiry date. This issues a new license revision; editing a catalog plan does not silently change licenses already issued to tenants. Deactivation blocks tenant routing and new control-plane logins; activation requires an unexpired subscription. Existing tenant JWTs are rejected by routing after the control-plane invalidation propagates. Serve the portal and API over HTTPS outside localhost.

Move and cold storage still require the replication and hydration setup described in `tenant-moves.md` and `cold-tenants.md`. The portal shows jobs and allows supported cancellation.

## API

- `POST /platform/auth/login` accepts username/password and returns a short-lived MFA challenge.
- `POST /platform/auth/totp/setup` accepts that challenge and returns an authenticator URI for first enrollment.
- `POST /platform/auth/totp/verify` accepts the challenge and a six-digit code and returns a session bearer token.
- `POST /platform/auth/logout` revokes a session.
- All management routes require the session token. `GET/POST /platform/plans` and `PUT /platform/plans/{code}` manage the plan catalog.
- `PUT /platform/tenants/{id}/subscription` assigns an active plan and future `endsAt`. `PUT /platform/tenants/{id}/enabled` activates or deactivates a tenant.
- Existing tenant, cell, timezone, administrator, move, cold, wake and cancellation routes remain available to authenticated operators.

## Verification

```sh
pnpm --filter @gaming-cafe/admin typecheck
pnpm --filter @gaming-cafe/admin exec vitest run src/portal/Portal.test.tsx
cargo check --manifest-path apps/backend/Cargo.toml --bin gaming-cafe-api --bin platform_operator --tests
CONTROL_TEST_DATABASE_URL=postgres://... cargo test --manifest-path apps/backend/Cargo.toml --test platform_portal -- --ignored
```
