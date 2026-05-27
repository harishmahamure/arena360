# ADR-0003: Secrets management

**Status**: Accepted
**Date**: 2026-05-27
**Deciders**: Platform team (gaming-cafe consolidation working group)

## Context

The three source repositories currently leak secrets in multiple ways:

- `arena360-backend/.env` and `arena360-backend/.env.development` are
  committed to the working tree (they are not in `.gitignore`).
- `arena360-backend/helm/fastify/values*.yaml` contains plaintext
  database passwords, JWT secret, ZeptoMail token, and R2 access keys.
- The fallback `'secret'` string in
  `apps/backend/src/services/auth/auth.service.ts` (path in the new
  monorepo) means the backend will silently boot with a known-public
  JWT signing key if `JWT_SECRET` is unset.

For the consolidation, this needs to be fixed before any code moves,
because the act of doing a fresh `git init` does not erase secrets
already exposed in the old remotes — they remain in archived
backup branches indefinitely.

We also need a uniform story for how secrets reach **each runtime**:

- Local dev: developer's machine, loading from a `.env` file.
- CI: GitHub Actions runner, loading from repository / org secrets.
- Production cluster: Kubernetes pod, loading from K8s `Secret`.

## Decision

We adopt a single, layered secrets policy across the monorepo:

1. **Source of truth per environment**:
   - **Local dev**: a developer-private `.env` file per app, derived
     from `env.template`. Never committed.
   - **CI**: GitHub Actions repository secrets (and environment
     secrets for prod jobs), referenced as `${{ secrets.NAME }}` in
     workflows.
   - **Cluster**: Kubernetes `Secret` objects, referenced by the
     backend deployment via `envFrom: secretRef:
     gaming-cafe-backend-secrets`. No plaintext credentials in
     `values*.yaml`.

2. **`.gitignore` rules** (root and per-app where useful):

   ```
   .env
   .env.*
   !.env.template
   !env.template
   ```

   The only env-shaped files allowed in version control are templates
   (with placeholder values).

3. **Fail-fast at boot**. `apps/backend/src/config/configuration.ts`
   validates that every required secret is present and non-empty
   before the Nest application is constructed. The `'secret'` JWT
   fallback in `auth.service.ts` is removed
   (consolidation plan, `jwt-fail-fast` todo). The required set at
   minimum:

   - `JWT_SECRET` (>= 32 bytes)
   - `DB_HOST`, `DB_PORT`, `DB_USER`, `DB_PASSWORD`, `DB_NAME`
   - `ZEPTOMAIL_TOKEN`
   - `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, `R2_BUCKET`,
     `R2_ENDPOINT`

   Missing or empty → process exits with code 1; no HTTP listener
   bound. See acceptance criteria for `US-AUTH-004` in
   `docs/REQUIREMENTS.md`.

4. **CI uses GitHub Actions secrets exclusively** for credentialed
   steps (image registry push, Helm deploy with kubeconfig, codegen
   against a backend that needs a DB). No secret value is ever
   echoed into a log line; `mask` annotations are applied where
   GitHub doesn't auto-mask.

5. **Cluster uses K8s Secrets via `envFrom`**:

   ```yaml
   spec:
     containers:
       - name: backend
         envFrom:
           - secretRef:
               name: gaming-cafe-backend-secrets
         env:
           # Only NON-SECRET config here.
           - name: NODE_ENV
             value: production
           - name: PORT
             value: "3000"
   ```

   The `Secret` itself is provisioned out-of-band (sealed-secrets,
   ExternalSecrets operator, or `kubectl create secret` from the
   ops runbook — orthogonal to this ADR).

## Rotation list and current state

The following secrets are considered **exposed** because they appear
in plaintext in committed files in at least one of the source repos
and **must be rotated before Phase 1** of the consolidation:

| Secret | Where exposed today | New home | Owner | Status |
|---|---|---|---|---|
| `DB_PASSWORD` | `arena360-backend/.env`, `arena360-backend/helm/fastify/values*.yaml` | K8s `Secret` `gaming-cafe-backend-secrets`; local `.env` | DBA / platform | needs rotation |
| `JWT_SECRET` | `arena360-backend/.env`, hard-coded `'secret'` fallback in `auth.service.ts` | K8s `Secret`; local `.env` | Platform | needs rotation |
| `ZEPTOMAIL_TOKEN` | `arena360-backend/.env`, Helm values | K8s `Secret`; local `.env` | Platform | needs rotation |
| `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY` | `arena360-backend/.env`, Helm values | K8s `Secret`; local `.env` | Platform | needs rotation |
| GitHub container registry token (`GHCR_TOKEN` / `CR_PAT`) | `arena360-backend/.github/workflows/*.yml` if any | GitHub Actions secret only | DevOps | review |
| Kubeconfig used by deploy workflows | GitHub Actions secret today (verify) | unchanged | DevOps | review |

**Rotation procedure (per secret)**:

1. Generate a new value (`openssl rand -hex 32` for `JWT_SECRET`;
   rotate the credential at the issuer for cloud keys).
2. Update the K8s `Secret` and roll the backend deployment.
3. Update the GitHub Actions secret (if used at deploy time).
4. Update developer `.env` files (broadcast on the team channel).
5. Decommission the old value at the issuer (revoke the old R2 key,
   delete the old DB user/role, etc.).

The rotation is gated **before** any code moves so that the secrets
left behind in the archived source-repo backups are already invalid
by the time those archives could plausibly leak.

## Consequences

### Positive

- No plaintext credentials in any tracked file in the new repo.
- Boot-time failure for misconfiguration replaces silent-bad-default
  behaviour; misconfigured deploys never go live.
- One pattern (env vars at the boundary, secrets from K8s `Secret`)
  is easy to teach and audit.
- Rotation is a documented operation, not a forensic exercise.

### Negative

- Developers must obtain credentials out-of-band (1Password vault,
  team-shared secure note). The friction is real and intentional.
- A missing local env var now stops the backend from booting; this is
  occasionally surprising on first run. Mitigation: `env.template`
  lists every required key with a comment.
- We commit to maintaining the rotation list in this ADR (or a linked
  runbook) as the secret inventory grows.

### Risks

- **Risk**: a contributor commits a `.env` by mistake.
  **Mitigation**: `.gitignore` covers it; pre-commit hook
  (`pnpm dlx lint-staged` or `husky`) scans for `.env` additions;
  GitHub Actions secret-scanning is enabled at the org level.
- **Risk**: a developer copy-pastes a real secret into a chat or PR.
  **Mitigation**: rotate immediately; treat the secret as burned;
  refresh per the rotation procedure above.
- **Risk**: K8s `Secret` is base64, not encrypted at rest by default.
  **Mitigation**: cluster-level etcd encryption at rest is enabled
  (verify with the ops runbook); for stronger guarantees, adopt
  sealed-secrets or ExternalSecrets in a follow-up ADR.

## Alternatives considered

### Commit encrypted secrets (e.g. `sops` + age)

- Pros: single source of truth lives next to code; CI decrypts at
  deploy time.
- Cons: another tool to learn and key-manage; less industry-standard
  than K8s `Secret` + GitHub Actions secrets; key compromise means
  silent decryption of history.
- **Why rejected**: not justified at our scale. May revisit if the
  secret inventory grows.

### Pull all secrets from a cloud secret manager (AWS / GCP) at boot

- Pros: rotation is centralised; audit log is built-in.
- Cons: adds a cloud-vendor dependency we don't otherwise have;
  requires VPC/network setup; extra failure mode at boot.
- **Why rejected**: K8s `Secret` is sufficient for one cluster
  serving one cafe. Reconsider if we ever support multi-cluster.

### Just `.env` files, forever

- Pros: simple.
- Cons: this is the failed status quo.
- **Why rejected**: this is what we are fixing.

## Implementation notes

- The `.gitignore` block above is added in Phase 1 of the
  consolidation (see `docs/MIGRATION.md` table 4).
- Removal of the `'secret'` fallback and addition of fail-fast
  validation is tracked as the `jwt-fail-fast` todo in the
  consolidation plan.
- Helm refactor (move env vars to `envFrom: secretRef`) is tracked as
  `helm-secrets` in the consolidation plan.
- The rotation list above is the authoritative checklist for the
  Phase 0 secret rotation gate.

## References

- ADR-0001: Turborepo + pnpm (where `.npmrc` is defined).
- `docs/REQUIREMENTS.md` `US-AUTH-004` (fail-fast acceptance criteria).
- Consolidation plan: `rotate-secrets`, `helm-secrets`,
  `jwt-fail-fast` todos.
