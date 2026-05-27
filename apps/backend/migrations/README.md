# Migrations

This Rust backend connects to an existing Postgres database whose schema
was originally created by TypeORM migrations in the NestJS backend.

**We maintain migrations here** for schema changes introduced by the Rust backend.
Create raw SQL migration files in this directory using:

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

For compile-time query checking in CI (offline mode):

```bash
pnpm migration prepare
```

This generates `.sqlx/` query metadata which should be committed.
