# DRAFT-0039: Database reset and superadmin seed

**Status**: Proposed
**Date**: 2026-06-17
**Deciders**: Platform team

**Relates to**: [ADR-0009](0009-rust-axum-backend.md), [ADR-0027](0027-run-migrations-from-ci-runner.md)

## Context

Fresh deployments and local dev environments sometimes need a clean slate: all
transactional and reference data removed, with a single known admin account for
first login. No prior migration embedded user passwords; auth hashes bcrypt at
runtime only.

Operators requested one migration that deep-cleans every application table and
seeds bootstrap credentials (`superadmin` / `SuperAdmin@123`).

## Decision

1. Add SQL migration `reset_database_seed_superadmin` that:
   - `TRUNCATE`s all 38 application tables (excluding `_sqlx_migrations`) with
     `RESTART IDENTITY CASCADE`
   - Inserts one `users` row: username `superadmin`, role `admin`, bcrypt cost 12
     hash for `SuperAdmin@123`
2. Run unconditionally on `pnpm migration run` in any environment (no env guard).
3. Down migration is **irreversible** for truncated data; removes only the seeded
   superadmin row.
4. Do **not** re-seed canonical units, inventory locations, or staff users.

## Consequences

### Positive

- One command yields a predictable empty DB + admin login for provisioning.
- Fixed admin UUID simplifies audit self-reference (`createdBy` / `updatedBy`).

### Negative

- **Destructive and irreversible** — all devices, sessions, products, and history
  are permanently deleted when the migration runs.
- Bootstrap password is committed in SQL (acceptable for initial setup; rotate
  post-deploy is out of scope).
- Empty reference tables (units, configurations) until recreated via admin UI/API.

### Risks

| Risk | Mitigation |
|------|------------|
| Accidental run on production | ADR + migration header warning; backup before `migration run` |
| Hardcoded password in repo | Document rotation as follow-up hardening |
| Missing reference data after wipe | Expected; admin recreates as needed |

## Alternatives Considered

### Separate `psql` reset script outside migrations

Rejected: operator asked for a single migration applied via standard `migration run`.

### Re-seed canonical units and inventory locations

Rejected: scope is superadmin only.

### Environment guard (dev-only)

Rejected: operator chose unconditional run on any environment.

## Implementation Notes

- Migration: `apps/backend/migrations/20260614224008_reset_database_seed_superadmin.up.sql`
- Truncates all `public` tables except `_sqlx_migrations` via dynamic SQL (handles
  schema drift without listing every table name).
- Login: `POST /auth/login/admin` with `{ "username": "superadmin", "password": "SuperAdmin@123" }`

## References

- `apps/backend/src/services/auth_service.rs` — bcrypt verify on login
- `apps/backend/src/repositories/user_repo.rs` — users INSERT shape
