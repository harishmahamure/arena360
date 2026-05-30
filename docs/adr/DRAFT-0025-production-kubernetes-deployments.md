# DRAFT-0025: Production Kubernetes deployments

**Status**: Proposed
**Date**: 2026-05-30
**Deciders**: Platform team

**Relates to**: [ADR-0001](0001-monorepo-turborepo-pnpm.md), [ADR-0003](0003-secrets-management.md), [ADR-0009](0009-rust-axum-backend.md), [ADR-0013](0013-realtime-websocket-channel.md)

## Context

The project needs production deployment assets for the current monorepo shape:

- Rust/Axum backend in `apps/backend`, not the older Fastify service.
- Admin web frontend in `apps/admin`.
- Backend WebSocket traffic served from the same backend at `/realtime`.
- A Kubernetes PostgreSQL database with PgBouncer in front for app connection pooling.
- Separate frontend and backend deployments so either surface can be released independently.

The example workflow and values provided use an older Fastify-style backend and
inline secrets. This repo's accepted ADRs require production secrets to stay out
of source control, and changes under `infra/**` / `.github/workflows/**` require
an ADR before implementation.

## Decision

Add production Kubernetes deployment assets with these release boundaries:

| Component | Helm release | Image tag pattern | Purpose |
|-----------|--------------|-------------------|---------|
| Backend | `arena360-backend` | `backend-prod-<short-sha>` | Rust/Axum API and `/realtime` websocket |
| Frontend | `arena360-frontend-new` | `frontend-prod-<short-sha>` | Admin web app served under `/v2` |
| Database | `arena360-postgresql` | chart-managed | PostgreSQL plus PgBouncer |

### Image repository

Use Docker Hub repository:

```text
harishmahamure/arena360
```

The backend workflow builds `apps/backend/Dockerfile`.
The frontend workflow builds `apps/admin/Dockerfile`.

### Backend ingress and WebSocket

Backend exposes both HTTP API and WebSocket traffic through the same service:

- Host: `api.arena360.cloud`
- HTTP: `https://api.arena360.cloud/...`
- WebSocket: `wss://api.arena360.cloud/realtime`

Ingress must include websocket-safe proxy timeout/read timeout settings.

### Frontend ingress

The frontend release is `arena360-frontend-new` and is mounted at:

```text
https://admin.arena360.cloud/v2
```

The frontend chart must configure ingress path handling, nginx SPA fallback, and
Vite/static asset base behavior so deep links and assets under `/v2` work.

### Database and PgBouncer

Deploy PostgreSQL in Kubernetes with PgBouncer in front:

- PostgreSQL release: `arena360-postgresql`
- PgBouncer service: `arena360-pgbouncer`
- Database: `arena360`
- Backend connects through PgBouncer, not directly to the PostgreSQL pod.

Application database configuration should prefer `DATABASE_URL`, built from
secret-backed values and the PgBouncer service:

```text
postgres://<user>:<password>@arena360-pgbouncer.default.svc.cluster.local:6432/arena360
```

PostgreSQL must use persistent storage. PgBouncer defaults to transaction pooling
for backend API workloads.

### Secrets

Do not commit secret values in Helm values files or workflow YAML.

Production deployments reference Kubernetes Secrets and GitHub Actions secrets:

- `KUBE_CONFIG`
- `DOCKERHUB_USERNAME`
- `DOCKERHUB_TOKEN`
- backend runtime secrets such as `DATABASE_URL`, `JWT_SECRET`, `ZEPTOMAIL_TOKEN`
- storage secrets such as `STORAGE_ACCESS_KEY`, `STORAGE_SECRET_KEY` when object storage is enabled

## Consequences

### Positive

- Backend and frontend can deploy independently.
- WebSocket routing remains simple: one backend service owns `/realtime`.
- Backend uses the Rust service's actual environment contract.
- PgBouncer protects PostgreSQL from excessive direct app connections.
- Existing `infra/helm/admin` can remain untouched while the new frontend release
  uses `arena360-frontend-new`.

### Negative

- Adds three Helm areas and three manual deployment workflows.
- Database deployment is now a separate operational concern.
- Serving the frontend under `/v2` requires careful ingress/nginx/Vite base-path handling.

### Risks

| Risk | Mitigation |
|------|------------|
| Secrets accidentally committed | Values files only reference existing Kubernetes Secrets / GitHub Secrets |
| WebSocket disconnects through ingress | Add proxy read/send timeout annotations and test `/realtime` after deploy |
| Frontend asset paths break under `/v2` | Configure Vite base/nginx fallback and template the chart path |
| Database data loss | Enable PostgreSQL persistent volume by default |
| PgBouncer transaction pooling issue | Keep pool mode configurable in values |

## Alternatives considered

### Single workflow deploys all services

- Pros: Simple button for all production rollout.
- Cons: Backend, frontend, and database have different risk profiles. Rejected.

### Reuse existing `infra/helm/admin` release

- Pros: Smaller diff.
- Cons: User requested `arena360-frontend-new`; preserving existing admin chart
  avoids breaking current release names. Rejected.

### Direct PostgreSQL connection from backend

- Pros: Fewer moving parts.
- Cons: Less resilient under scaling; PgBouncer was explicitly requested. Rejected.

## Implementation notes

After acceptance:

1. Add `infra/helm/backend/**`.
2. Add `infra/helm/frontend/**` for `arena360-frontend-new`.
3. Add `infra/helm/postgresql/**` with PostgreSQL plus PgBouncer configuration.
4. Add `.github/workflows/deploy-production-backend.yml`.
5. Add `.github/workflows/deploy-production-frontend.yml`.
6. Add `.github/workflows/deploy-production-db.yml`.
7. Validate with `helm lint` and `helm template`.

## References

- [ADR-0001](0001-monorepo-turborepo-pnpm.md)
- [ADR-0003](0003-secrets-management.md)
- [ADR-0009](0009-rust-axum-backend.md)
- [ADR-0013](0013-realtime-websocket-channel.md)
