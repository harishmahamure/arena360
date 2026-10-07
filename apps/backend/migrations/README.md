# Database migrations

Only two schema families are active:

- `control/`: PostgreSQL routing, ownership, global staff identities, memberships and recovery metadata.
- `tenant/`: operational SQLite schema embedded into each owning-cell process.

The shared operational PostgreSQL migrations are retired. Existing files remain available in Git history.

```bash
pnpm migration generate add_control_metadata --target control
pnpm migration generate add_tenant_table --target tenant
pnpm migration run --target control
pnpm migration info --target control
```

Control commands require `CONTROL_DATABASE_URL`; they do not use the old operational
`DATABASE_URL` or `DB_*` settings. Startup also embeds and applies control migrations.
Tenant schema changes use sequential numbered files, followed by the lease-fenced tenant
migration orchestrator. Do not run PostgreSQL migration tools against a tenant file.
Review generated SQL before applying it; migration recovery and snapshot gates remain in
`TenantMigrationRunner`.
