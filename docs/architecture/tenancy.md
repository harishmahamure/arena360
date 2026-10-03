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

## Location catalogs and reporting

Migrations `20261003180000` through `20261003210000` add catalog availability,
location price overrides, reporting attribution, and the updated generated location
administrator role. Apply the migrations with the analytics worker stopped, then
restart the API and the upgraded worker. The analytics migration queues refreshed
rows with newer CDC versions; no manual backfill is needed. Existing sessions for
members whose generated role changed must sign in again.

Products and plans have `locationIds`: an empty array shares the catalog entry with
all locations in its business; a nonempty array limits availability to those
locations. The name and configuration are shared by the selected locations.
Organization administrators manage business-wide entries. A location administrator
can edit an entry only when every location using it grants the write permission.
Location price overrides can be edited independently for a shared entry, preserving
prices at locations the editor cannot manage. Removing an override restores the
catalog price. Published pricing rules apply after these base prices. POS and kiosk
menus and checkout enforce availability and price overrides on the server.

Pricing policies can target all locations, one location, or a selected list.
Updating or publishing a policy requires permission at every targeted location;
shared policies require an organization administrator. Settings retain organization
defaults with location overrides. The configuration UI can copy a change to several
locations, using each destination's own revision and audit record.

The workspace location selector sends `X-Location-Id`, including through the RPC
bridge. An unselected location means all accessible locations for reports; server
queries resolve current permission grants and restrict aggregation accordingly.
Organization administrators can aggregate the whole organization. `/stats` and the
finance report are tenant-scoped, including ClickHouse reads and cache keys, and
are available to other organizations. Other legacy operational routes retain the
existing tenant rollout restrictions described above.

Reports project organization IDs and location relationships into ClickHouse.
Transactions record their venue; usage sessions snapshot the device's venue so a
later device move does not reattribute historical usage. Finance CSV exports name
the reporting locations. Customer and wallet counts represent the customers who
visited the selected locations, so shared customer wallets are not additive across
location reports.
