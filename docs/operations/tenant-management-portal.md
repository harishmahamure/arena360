# Tenant management portal

The internal portal manages platform operators, plans, tenants, cells and tenant administrators. Operator accounts are separate from tenant business accounts. It builds as a separate static image and Kubernetes release from the staff panel.

## Start locally

For the split deployment, configure the root `.env` as described in [two Rust services](two-rust-services.md), then start each service in its own terminal:

```sh
PORT=3001 pnpm storage:dev
# In a second terminal (gateway uses PORT=3000 from .env):
pnpm gateway:dev
# In a third terminal:
pnpm portal:dev
# Optional: the customer-facing staff panel
pnpm admin:dev
```

The gateway needs `CONTROL_DATABASE_URL`, `JWT_SECRET`, `STORAGE_SERVICE_TOKEN`, `STORAGE_SERVICE_URL` and `STORAGE_CELL_ID`. Storage needs the same database/auth settings, a matching `ARENA_CELL_ID`, and `TENANT_DATA_DIR`. Register the storage origin (typically `http://localhost:3001`) in the portal.

For the combined compatibility backend, configure `CONTROL_DATABASE_URL`, `ARENA_CELL_ID`, `TENANT_DATA_DIR`, `JWT_SECRET` and `PORT`, then use `pnpm backend:dev` instead of the two service commands. Register that backend's origin (typically `http://localhost:3000`). The control migration creates the platform operator and plan tables. `PLATFORM_ADMIN_TOKEN` is no longer used.

Create the first operator with a password sent through stdin:

```sh
read -rs 'OPERATOR_PASSWORD?Platform password: '; printf '\n'
printf '%s\n' "$OPERATOR_PASSWORD" | CONTROL_DATABASE_URL=postgres://USER:PASSWORD@127.0.0.1:5432/arena_portal cargo run -p arena360-tools --bin platform_operator -- operator
unset OPERATOR_PASSWORD
```

The password must be 12–72 bytes. Run `pnpm portal:dev` and open `http://localhost:5174/`. Sign in with the operator username and password. First sign-in presents a QR code and manual key; add it to an authenticator app and verify a six-digit code before any management endpoint is accessible. Later sign-ins require a fresh code. Sessions last 12 hours, stay in browser memory and can be revoked by signing out. Five failed password attempts lock that account for 15 minutes; each MFA challenge allows five code attempts and expires after 10 minutes.

For a demo operator, set `DEMO_PLATFORM_PASSWORD` to a unique 12–72 byte password when running `pnpm demo:seed`. The seed creates `demo.platform` after the demo tenant is ready. It leaves an existing account and its enrolled TOTP unchanged and fails if the supplied password differs. `DEMO_PLATFORM_USERNAME` can choose another username. No demo password is built into the image or seeded in production.

## Create a tenant, step by step

1. Open the local [tenant portal](http://localhost:5174/) and sign in with your platform operator, such as `harishmahamure`. Complete the authenticator setup on first sign-in, or enter the current six-digit code on later sign-ins. This account manages the platform; it is separate from the customer administrator.
2. On **Tenants**, check **Tenant creation checklist**. The API must have provisioning configured, the default database worker must be registered and active, and the **Trial** plan must be active. Use **Register database worker** if registration is missing. Give the worker a recognizable name; keep the prefilled ID and address. When an older backend does not supply an address, enter the configured storage service's reachable HTTP(S) origin yourself. Registration records the worker; it does not start it.
3. Click **Create tenant**. Enter the customer's business name. The portal generates the unique slug; change it if necessary. Choose the business timezone and trial duration (1–365 days), then submit. The backend creates the PostgreSQL tenant/subscription records and provisions the tenant's SQLite database with current migrations on its worker.
4. In **Create tenant administrator**, use a unique username and set and confirm a password of at least 12 characters (maximum 72 UTF-8 bytes). The suggested username is `<slug>.admin`. These are the credentials the customer uses in the staff panel. Share the password directly with that administrator.
5. If administrator creation fails, correct the error and retry this step. The tenant is already saved; retries only create its administrator. **Finish later** opens tenant details, where **Create administrator** lets you complete setup later.
6. When setup is complete, click **Open business admin** and sign in with the new tenant administrator. In local development this opens [the staff panel](http://localhost:5173/). Add venues, devices, staff and products there. Start the staff panel with `pnpm admin:dev` if it is not already running.

The portal uses the current `/platform/*` API for both the combined local backend and the split gateway/storage deployments. The overview response includes optional provisioning mode/address metadata to help register the correct worker; the portal also supports backends without these fields. In the split deployment, the worker address is the storage service address, not the gateway address. For setup and environment variables, see [two Rust services](two-rust-services.md). Point `PLATFORM_API_PROXY_TARGET` at the gateway when running the portal against that deployment.

Tenant provisioning applies all tenant migrations and commits the defaults before acquiring ownership and marking the tenant `ACTIVE`. It seeds eleven units; the Administrator, Counter operator, and Location administrator roles; and six templates (Venue manager, Counter operator, Kitchen operator, Finance reviewer, Auditor, and Location administrator). The Administrator includes every permission in the access catalog, including location setup. Location administrator grants match the staff panel's location administrator workflow.

Settings use the code-owned catalog defaults plus validated initial overrides; provisioning does not copy platform settings into each tenant. Retries preserve customized units, role permissions, and settings, and restore missing defaults. A conflicting default name/ID or incompatible role/template type stops provisioning and rolls back the seed transaction. Correct that conflict and retry the same tenant request. Existing customized role permissions are not upgraded by retrying provisioning.

A fresh backup/hydration heartbeat is required for tenant moves and cold-storage recovery, not for ordinary tenant creation. The worker table labels this separately as **Backup restore**.

## Separate deployment

`pnpm portal:build` builds the dedicated `apps/tenant` application into `apps/tenant/dist`. `pnpm admin:build` builds the staff panel separately. The [portal Dockerfile](../../apps/tenant/Dockerfile) serves the portal at `/`; Nginx forwards `/platform/*` to the backend through `PLATFORM_API_UPSTREAM` so browser requests stay on the portal origin.

The [platform portal Helm chart](../../infra/helm/platform-portal) installs an independent Deployment, Service and Ingress. Production values use `platform.arena360.cloud` and the in-cluster backend service. Set DNS for that hostname and provide the existing Docker Hub and Kubernetes GitHub secrets. The [deployment workflow](../../.github/workflows/deploy-production-platform-portal.yml) is manually dispatched and publishes a separate `portal-prod-<commit>` image; it does not modify the staff frontend release. Override `PLATFORM_PORTAL_HOST` in GitHub repository variables if the portal uses another hostname.

Use **Cells** to register the database worker's exact `ARENA_CELL_ID` and origin. Use **Tenants** to provision a tenant and its first business administrator. Use **Plans** to create or edit a reusable code, entitlements and grace period. The trial plan exists by default. In a tenant workspace, assign an active plan and future expiry date. This issues a new license revision; editing a catalog plan does not silently change licenses already issued to tenants. Deactivation blocks tenant routing and new control-plane logins; activation requires an unexpired subscription. Existing tenant JWTs are rejected by routing after the control-plane invalidation propagates. Serve the portal and API over HTTPS outside localhost.

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
pnpm --filter @gaming-cafe/tenant typecheck
pnpm --filter @gaming-cafe/tenant test
pnpm portal:build
cargo check --workspace --all-targets
CONTROL_TEST_DATABASE_URL=postgres://... cargo test -p arena360-core --test platform_portal -- --ignored
```
