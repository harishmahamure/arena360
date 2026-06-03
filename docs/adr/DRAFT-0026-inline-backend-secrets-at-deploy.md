# DRAFT-0026: Inline backend secrets at deploy time (no K8s Secret object)

**Status**: Proposed
**Date**: 2026-06-03
**Deciders**: Platform team
**Supersedes**: partially overrides [ADR-0003](0003-secrets-management.md) §5 (cluster delivery mechanism)
**Relates to**: [ADR-0003](0003-secrets-management.md), [DRAFT-0025](DRAFT-0025-production-kubernetes-deployments.md), [ADR-0009](0009-rust-axum-backend.md)

## Context

The production backend deploy workflow (`.github/workflows/deploy-production-backend.yml`)
currently creates a Kubernetes `Secret` (`arena360-backend-secrets`) from GitHub
Actions secrets via a `kubectl create secret ... --from-literal=...` step. The
Helm chart (`infra/helm/backend`) then wires pod env from that Secret through
`envFromSecrets` (`secretKeyRef`), and the inline migration `Job` reads
`MIGRATION_DATABASE_URL` from the same Secret.

The maintainer wants to remove the standalone Kubernetes `Secret` object and
instead inject the GitHub Actions secret values directly into the rendered
manifests at deploy time ("just refer and use" the GitHub secrets), eliminating
the separate `kubectl create secret` step.

This changes the **cluster delivery mechanism** defined in ADR-0003 §5
(K8s `Secret` via `envFrom`), so an ADR is required before implementation.

## Decision

Deliver backend runtime secrets by injecting GitHub Actions secret values
directly into the rendered Kubernetes manifests at deploy time, with **no
standalone `Secret` object**:

1. **Remove** the "Upsert backend secret" step from the backend deploy workflow.
2. **Backend Deployment (Helm)**: add an optional `inlineSecretEnv` map to
   `infra/helm/backend/values.yaml`, rendered by `templates/deployment.yaml` as
   literal `env:` entries (`name`/`value`) in the pod spec. Disable / remove the
   `envFromSecrets` (`secretKeyRef`) wiring for production. The workflow supplies
   values at deploy time, e.g.:

   ```bash
   helm upgrade --install arena360-backend ./infra/helm/backend \
     --values ./infra/helm/backend/values-prod.yaml \
     --set-string inlineSecretEnv.DATABASE_URL="${{ secrets.DATABASE_URL }}" \
     --set-string inlineSecretEnv.JWT_SECRET="${{ secrets.JWT_SECRET }}" \
     --set-string inlineSecretEnv.ZEPTOMAIL_TOKEN="${{ secrets.ZEPTOMAIL_TOKEN }}" \
     ...
   ```

3. **Migration `Job`**: replace the `secretKeyRef` env source with a direct
   `value:` interpolated from the GitHub secret in the heredoc:

   ```yaml
   env:
     - name: DATABASE_URL
       value: '${{ secrets.MIGRATION_DATABASE_URL }}'
   ```

No secret value is committed to any tracked file; values originate from GitHub
Actions secrets and are interpolated only at workflow run time.

## Consequences

### Positive

- One fewer deploy step; the GitHub secret is the single source, injected directly.
- No separate `Secret` object to reconcile or drift from the GitHub secrets.

### Negative / Risks

- **Secrets become visible in plaintext in the rendered Deployment and Job
  specs** (`kubectl get deploy/job -o yaml`, `helm get manifest`, Helm release
  history stored as a Secret). This is weaker than a dedicated `Secret` object
  consumed via `secretKeyRef`, and is the explicit trade-off ADR-0003 §5 avoided.
- Helm stores release manifests (including the inlined values) in its release
  Secret, so the values still land in etcd — but now also embedded in the
  Deployment object and rollout history.
- Anyone with `get deployment`/`get job` RBAC (broader than `get secret`) can
  read the credentials.
- Diverges from the uniform "secrets via K8s `Secret`" pattern in ADR-0003,
  increasing cognitive load.

### Mitigation

- Keep etcd encryption-at-rest enabled (ADR-0003 risk note).
- Scope RBAC so manifest/release read access is restricted.

## Alternatives considered

### Keep the K8s `Secret` (status quo, ADR-0003) — smallest change, no ADR

- Pros: matches ADR-0003; secrets only readable via `get secret`; the existing
  `--from-literal='${{ secrets.X }}'` step *already* "refers to and uses" the
  GitHub secrets.
- Cons: retains the step the maintainer wants gone.
- **This is the smallest alternative that requires no ADR.**

### Helm-managed `Secret` template (chart owns the Secret)

- Pros: keeps `secretKeyRef`/`Secret` security posture; consolidates creation
  into the Helm release instead of a separate `kubectl` step.
- Cons: still a chart change (ADR); still a `Secret` object.

### ExternalSecrets / sealed-secrets operator

- Pros: strongest posture, centralised rotation.
- Cons: new infra dependency; out of scope for one cluster (ADR-0003 deferred this).

## References

- [ADR-0003](0003-secrets-management.md) §5 (cluster secret delivery)
- [DRAFT-0025](DRAFT-0025-production-kubernetes-deployments.md) (secrets sub-section)
- `infra/helm/backend/values.yaml` (`envFromSecrets`)
- `.github/workflows/deploy-production-backend.yml`
