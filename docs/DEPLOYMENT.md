# Deployment and operations

This document describes the deployment mechanisms that exist in the repository.
It does not replace venue-specific secret management, backup, networking, or
device-management procedures.

## Server deployment

The backend and staff console have production Dockerfiles and Kubernetes Helm
charts.

| Component | Image/runtime | Helm chart | Production workflow |
|---|---|---|---|
| Backend | Debian image containing the Rust API | `infra/helm/backend` | `deploy-production-backend.yml` |
| Staff console | Nginx serving the Vite build | `infra/helm/frontend` | `deploy-production-frontend.yml` |
| PostgreSQL + PgBouncer | PostgreSQL 16 and PgBouncer | `infra/helm/postgresql` | `deploy-production-database.yml` |
| Redis | Redis 7 | `infra/helm/redis` | `deploy-production-redis.yml` |

The checked-in production values expose the staff console at
`staff.arena360.cloud` and the API/WebSocket service at
`api.arena360.cloud`. GitHub Actions deployment workflows are manually
dispatched.

The backend workflow can optionally run SQLx migrations before rollout. It
builds a commit-addressed image, injects runtime secrets through a temporary
Helm values file, and performs an atomic Helm upgrade. Database, Redis, frontend,
and backend infrastructure have separate workflows.

## Required backend secrets

Production requires:

- `DATABASE_URL`
- `JWT_SECRET` with at least 32 characters

Configure these when the corresponding feature is enabled:

- `REDIS_URL`
- `STORAGE_ENDPOINT`
- `STORAGE_BUCKET`
- `STORAGE_ACCESS_KEY`
- `STORAGE_SECRET_KEY`
- `STORAGE_PUBLIC_URL`
- `ZEPTOMAIL_TOKEN`

The backend fails startup when PostgreSQL or the JWT secret is invalid. Redis is
different by design: an unavailable Redis instance produces a warning and the
process continues with caching disabled.

## Health and diagnostics

| Endpoint | Meaning |
|---|---|
| `GET /health/live` | Process liveness |
| `GET /health/ready` | Dependency readiness |
| `GET /health` | Legacy aggregate health endpoint |
| Staff console `/health` | Nginx liveness |
| Staff console `/ready` | Nginx readiness |

The backend writes structured tracing output to standard output. Kubernetes
probes use the liveness and readiness endpoints. Swagger UI is intentionally
disabled when `NODE_ENV`, `RUST_ENV`, or `ENVIRONMENT` equals
`production`.

## Windows kiosk release

The kiosk builds as a per-machine NSIS installer from the Tauri configuration.

```bash
pnpm --filter @gaming-cafe/kiosk tauri:build
```

The `kiosk-ci.yml` workflow tests Windows builds. The manually dispatched
`kiosk-release.yml` workflow versions the application, builds release
artifacts, and can publish signed updater metadata when the updater credentials
are configured.

Important runtime behavior:

- WebView2 is embedded as a bootstrapper and checked again at application
  startup.
- The application provides app-level keyboard/taskbar lockdown, session process
  tracking, and cleanup.
- Windows Secure Attention Sequence behavior still requires operating-system
  policy; a user-mode application cannot intercept Ctrl+Alt+Delete.
- Auto-update checks run only while the kiosk is idle, not during a player
  session.
- The primary diagnostic log is
  `%ProgramData%\Arena360\kiosk.log`; one rotated backup is kept.

Production distribution should configure both the Tauri updater signing key and
Windows Authenticode signing. These are independent trust mechanisms.

## Provisioning station clients

The Windows kiosk is provisioned using an administrator account. It calls the
device-provisioning endpoint, stores the returned device identity and token,
then connects to the device's realtime channel and proceeds to station setup.
An administrator configures the local launch allow-list and presentation media
there.

Clearing application data removes the local device identity and returns the
station to provisioning. Treat that as a controlled re-enrollment operation.

## Operational safeguards

- Run database backups independently of application deployments and verify
  restore procedures.
- Apply migrations before deploying code that depends on them.
- Keep `JWT_SECRET`, database credentials, storage credentials,
  Authenticode certificates, and updater private keys outside the repository.
- Verify WebSocket proxy timeouts for long-lived `/realtime` connections.
- Monitor PostgreSQL capacity first; Redis loss degrades performance, while
  PostgreSQL loss makes the API unready.
- Roll out station updates while devices are idle and retain the previous
  installer for recovery.
