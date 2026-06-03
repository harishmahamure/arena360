# ADR-0027: Run database migrations from the CI runner (drop in-cluster Job)

**Status**: Accepted
**Date**: 2026-06-03
**Deciders**: Platform team
**Supersedes**: partially overrides the migration mechanism in [ADR-0026](0026-inline-backend-secrets-at-deploy.md) / [DRAFT-0025](DRAFT-0025-production-kubernetes-deployments.md)
**Relates to**: [ADR-0009](0009-rust-axum-backend.md), [ADR-0003](0003-secrets-management.md)

## Context

The production backend deploy workflow
(`.github/workflows/deploy-production-backend.yml`) previously ran schema
migrations by:

1. Uploading `apps/backend/migrations` into the cluster as a `ConfigMap`
   (`arena360-backend-migrations`).
2. Applying a one-off `Job` (`arena360-backend-migrate`) that ran
   `rust:1-slim`, **compiled `sqlx-cli` from source via `cargo install`** on
   every deploy, mounted the ConfigMap, and ran `sqlx migrate run`.
3. `kubectl wait`-ing for the Job, which was re-created (delete + apply) on the
   next run.

This was slow (sqlx-cli recompiled each deploy, several minutes) and carried
operational overhead (ConfigMap upload, Job lifecycle, wait/delete).

The PgBouncer pooler is already exposed outside the cluster via a NodePort
(`infra/helm/postgresql/values-prod.yaml`: `pgbouncer.externalService` →
`type: NodePort`, `port: 5433`, `nodePort: 30433`). The `MIGRATION_DATABASE_URL`
GitHub Actions secret points at a runner-reachable endpoint (same model as the
existing `DATABASE_URL` in `apps/backend/.env`, which uses a NodePort host).

## Decision

Run migrations on the CI runner against the externally reachable database, and
remove the in-cluster migration artefacts. As implemented in
`.github/workflows/deploy-production-backend.yml`:

```yaml
- name: Install sqlx-cli
  uses: taiki-e/install-action@v2
  with:
    tool: sqlx-cli

- name: Run database migrations
  env:
    DATABASE_URL: "${{ secrets.MIGRATION_DATABASE_URL }}"
  run: sqlx migrate run --source apps/backend/migrations
```

- The migrations `ConfigMap` step and the migration `Job` (heredoc +
  `kubectl wait` + `kubectl delete job`) are removed.
- `MIGRATION_DATABASE_URL` is consumed only on the runner; it is not placed in
  any cluster object (the running backend uses `DATABASE_URL`, now inlined via
  Helm `inlineSecretEnv` per ADR-0026).
- Migrations run **before** `helm upgrade`, so the schema is ready before the
  new backend version rolls out.

## Consequences

### Positive

- Much faster deploys: no per-run `cargo install sqlx-cli` compile.
- Fewer moving parts: no migrations ConfigMap, no Job lifecycle / wait / delete.
- Migrations run from the exact source in the checkout (single source of truth).

### Negative / Risks

- The CI runner connects to the production DB over the public NodePort. The
  runner already holds the DB credentials and full `KUBE_CONFIG`, so this is a
  small trust-boundary delta, but it requires the DB NodePort to remain
  reachable from GitHub-hosted runners.
- If the NodePort is later firewalled to internal ranges, the runner cannot
  connect and the deploy fails at the migration step.
- Loses the in-cluster `Job` audit trail for each migration run.

### Mitigation

- Consider pinning `sqlx-cli` to the `0.8` line (matching the backend crate) for
  reproducibility: `tool: sqlx-cli@0.8.x`.
- If the NodePort must be locked down, fall back to `kubectl port-forward` to
  `svc/arena360-postgresql-pgbouncer` and run `sqlx migrate run` against
  `localhost` (still "directly", no Job).

## Alternatives considered

### Keep the in-cluster Job, just speed it up

- Replace `cargo install sqlx-cli` with a prebuilt binary inside the Job image,
  keeping execution in-cluster.
- Pros: preserves in-cluster execution and audit trail; no CI→DB network need.
- Cons: retains the ConfigMap + Job lifecycle.

### `kubectl run` ephemeral pod instead of a Job

- Pros: no Job object/wait/delete; still in-cluster.
- Cons: still in-cluster artefact management; still needs migrations mounted.

### Migrations embedded in the backend binary (`sqlx::migrate!()` on boot)

- Pros: no separate migration step.
- Cons: couples rollout to migration success; concurrent replicas race on
  startup; larger change to the ADR-0009 boot path.

## References

- `.github/workflows/deploy-production-backend.yml`
- `infra/helm/postgresql/values-prod.yaml` (`pgbouncer.externalService` NodePort)
- `apps/backend/Cargo.toml` (`sqlx = "0.8"`)
- [ADR-0026](0026-inline-backend-secrets-at-deploy.md),
  [DRAFT-0025](DRAFT-0025-production-kubernetes-deployments.md)
