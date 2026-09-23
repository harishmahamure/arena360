# Database migrations

SQLx migrations in this directory define the Arena360 PostgreSQL schema.
Create a migration from the repository root with:

```bash
pnpm migration generate <description>
```

Apply pending migrations:

```bash
pnpm migration run
```

Other commands:

```bash
pnpm migration info
pnpm migration revert
pnpm migration prepare
```

To refresh SQLx offline query metadata:

```bash
pnpm migration prepare
```

Review generated SQL before applying it. Where a reversible change is practical,
keep the matching `.down.sql` file beside the forward migration.
