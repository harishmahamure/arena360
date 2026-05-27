# Migrations

This Rust backend connects to an existing Postgres database whose schema
was originally created by TypeORM migrations in the NestJS backend.

**We do NOT maintain migrations here.** The schema is treated as an
external dependency. If schema changes are needed, create raw SQL
migration files in this directory using:

```bash
sqlx migrate add <description>
```

For compile-time query checking in CI (offline mode):

```bash
cargo sqlx prepare
```

This generates `sqlx-data.json` which should be committed.
