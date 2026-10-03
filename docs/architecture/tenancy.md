# Shared-table tenancy

Organizations are the tenant boundary. Users remain global identities and join
businesses through `organization_memberships`. Venue locations belong to one
organization.

Migrations `20261003150000` through `20261003170000` add `organizationId`
to operational tables, seed a separate editable unit catalog for each
organization, and require tenant-owned foreign keys to stay inside one
organization. Player and shift records also require a membership in their
organization. Existing single-business rows are assigned to the bootstrap
organization. Rows already linked to a secondary organization's venue or
pricing rules inherit that organization's ID during backfill.

Global tables are `users`, `organizations`, authentication challenges,
realtime transport tables, and the SQLx migration ledger. Access assignments,
memberships, venue locations, pricing sets, and settings already had an
organization key before these migrations.

The default `organizationId` on newly scoped tables is a compatibility
measure for existing single-business writes. New tenant-aware writes must pass
the selected organization ID explicitly. The API still blocks non-default
operational routes in `access/routes.rs`; query scoping and removal of the
compatibility defaults are required before enabling those routes for other
businesses. Database foreign keys protect relationships, while runtime reads
still require tenant predicates.
