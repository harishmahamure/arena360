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
2. Applying a one-off `Job` (`arena360-backend-migrate`) that runs
   `rust:1-slim`, **compiles `sqlx-cli` from source via `cargo install`** on
   every deploy, mounts the ConfigMap, and runs `sqlx migrate run`.
3. `kubectl wait`-ing for the Job, which is then re-created (delete + apply) on
   the next run.

This is slow (sqlx-cli is recompiled each deploy, typically several minutes) and
carries operational overhead (ConfigMap upload, Job lifecycle, wait/delete).

The PgBouncer pooler is already exposed outside the cluster via a NodePort
(`infra/helm/postgresql/values-prod.yaml`: `pgbouncer.externalService` →
`type: NodePort`, `port: 5433`, `nodePort: 30433`). The `MIGRATION_DATABASE_URL`
GitHub Actions secret points at a runner-reachable endpoint (the same model as
the existing `DATABASE_URL` in `apps/backend/.env`, which uses a NodePort host).

The maintainer wants to run migrations **directly from the GitHub Actions
runner** instead of creating the in-cluster Job.

## Decision

Run migrations on the CI runner against the externally reachable database, and
remove the in-cluster migration artefacts:

1. **Remove** the "Upload backend migrations" `ConfigMap` step.
2. **Remove** the "Run database migrations" `Job` heredoc, `kubectl wait`, and
   `kubectl delete job`.
3. **Add** a step that installs a prebuilt `sqlx-cli` (no source compile) and
   runs the checked-out migrations directly:

   ```yaml
   - name: Install sqlx-cli
     uses: taiki-e/install-action@v2
     with:
       tool: sqlx-cli

   - name: Run database migrations
     env:
       DATABASE_URL: ${{ secrets.MIGRATION_DATABASE_URL }}
     run: sqlx migrate run --source apps/backend/migrations
   ```

`MIGRATION_DATABASE_URL` is consumed only by this CI step; no cluster workload
references it (the running backend uses `DATABASE_URL`).

Migrations run **before** `helm upgrade`, so the schema is ready before the new
backend version rolls out. `sqlx-cli` tracks the backend's `sqlx` crate (0.8).

## Consequences

### Positive

- Much faster deploys: no per-run `cargo install sqlx-cli` compile.
- Fewer moving parts: no migrations ConfigMap, no Job lifecycle / wait / delete.
- Migrations run from the exact source in the checkout (single source of truth).

### Negative / Risks

- The CI runner connects to the production DB over the public NodePort. The
  runner already holds the DB credentials and full `KUBE_CONFIG`, so this is a
  small trust-boundary delta, but it does require the DB NodePort to remain
  reachable from GitHub-hosted runners.
- Migrations no longer run inside the cluster network; if the NodePort is
  firewalled to internal ranges, the runner cannot connect (would need a
  `kubectl port-forward` tunnel or a self-hosted runner instead).
- Loses the in-cluster audit trail of a `Job` object for each migration run.

### Mitigation

- Pin `sqlx-cli` to the `0.8` line to match the backend crate.
- If the NodePort must be locked down, fall back to `kubectl port-forward` to
  `svc/arena360-postgresql-pgbouncer` and run `sqlx migrate run` against
  `localhost` (still "directly", no Job).

## Alternatives considered

### Keep the in-cluster Job, just speed it up — smaller change, no ADR

- Replace `cargo install sqlx-cli` with a prebuilt binary inside the Job image,
  keeping execution in-cluster.
- Pros: preserves the in-cluster execution model and audit trail; no new CI→DB
  network requirement.
- Cons: retains the ConfigMap + Job lifecycle the maintainer wants gone.
- **This is the smallest alternative that would not require an ADR.**

### `kubectl run` ephemeral pod instead of a Job

- Pros: no Job object/wait/delete; still in-cluster (no CI→DB requirement).
- Cons: still in-cluster artefact management; still needs migrations mounted.

### Migrations embedded in the backend binary (`sqlx::migrate!()` on boot)

- Pros: no separate migration step at all.
- Cons: couples rollout to migration success; concurrent replicas race on
  startup; larger change to ADR-0009 boot path. Out of scope here.

## References

- `.github/workflows/deploy-production-backend.yml`
- `infra/helm/postgresql/values-prod.yaml` (`pgbouncer.externalService` NodePort)
- `apps/backend/Cargo.toml` (`sqlx = "0.8"`)
- [DRAFT-0025](DRAFT-0025-production-kubernetes-deployments.md),
  [ADR-0026](0026-inline-backend-secrets-at-deploy.md)
