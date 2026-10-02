# Access management

Open **Administration → Access management** to manage roles, reusable templates, team membership, modules and the audit trail. Role/template and member forms use guided steps with a review before saving.

## Roles and templates

A member can have multiple roles. Their effective permissions are the union of assigned roles, restricted by enabled modules. Write/manage grants include the matching read grant. Templates are reusable starting points: create a role from a template, then assign that role. Templates cannot be assigned directly, and editing a template does not alter roles already created from it.

Seed templates cover venue managers, counter operators, kitchen operators, finance reviewers and auditors. Existing administrators and staff receive compatible initial roles. The legacy `admin`/`staff` database values remain account kinds for compatibility; they no longer determine a managed panel session's permissions. New team members are created through Access management.

Disabling a module removes its grants from every member while preserving data and role definitions. Access management cannot be disabled. Shift and cash-register modules cannot be disabled while a shift remains active. Review cross-module dependencies when designing roles: counter workflows need shifts, registers, players, products and transactions as appropriate. Broad venue event streams have separate grants; kitchen-only users can subscribe to the dedicated kitchen channel.

## Enforcement and session changes

The backend checks permissions, organization membership and the current effective permission set on requests. Unmapped panel routes deny access. Navigation and actions also follow explicit permissions. Changing a member's effective permissions invalidates their existing token and requires signing in again. Revoked WebSocket sessions stop receiving events and are disconnected by the periodic session check. Changes to roles, memberships and modules are audited. Concurrent edits are rejected using revisions; reload before retrying. The last active access administrator cannot be removed, and assigned roles cannot be deleted.

## Rollout

Apply `apps/backend/migrations/20261002190000_configurable_access.sql` before starting the updated backend, then deploy the matching admin frontend. The migration seeds existing organizations and installs initialization for new organizations/memberships. Existing panel tokens must sign in again to acquire managed permission claims. Back up the database before the normal production migration process. This implementation was exercised against an isolated QA database, not deployed to production.

Role definitions, assignments, module switches and audit records are organization-scoped. The existing operational ledger is still shared legacy storage without full tenant columns; operational endpoints are therefore restricted to its owning default venue. This change does not migrate that ledger or provide an organization switcher. Other organizations can manage their scoped access/settings but require a separate ledger migration before operational use.

Permission checks do not replace existing location/resource restrictions, shift requirements or financial validation. The audit page shows the latest 100 access changes.
