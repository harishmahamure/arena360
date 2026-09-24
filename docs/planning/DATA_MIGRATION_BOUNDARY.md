# Existing-data migration boundary

Existing Arena360 data will be migrated by a separate tool owned and delivered
by the user. That tool is not part of the product milestones in this plan and
does not create a backward-compatibility requirement for the new system.

## Ownership split

| Responsibility | New Arena360 system | Separate migration tool |
| --- | --- | --- |
| Canonical data model | Defines the final organization, location, station, identity, money, entitlement, session, inventory, audit, and configuration models | Does not preserve legacy shapes in the target system |
| Import contract | Publishes a versioned, documented input contract and validation rules | Produces records that conform to the selected contract version |
| Legacy extraction | No responsibility | Reads and interprets existing data sources |
| Transformation and mapping | Defines required semantics and invariants | Maps legacy identifiers, enums, relationships, and values into the new model |
| Validation | Rejects invalid or unsafe imports with actionable errors | Performs preflight checks and reports unmappable or ambiguous data |
| Execution | Provides an approved import boundary | Supports dry run, resumability, idempotency, batching, and retries |
| Reconciliation | Exposes target counts and integrity results | Produces source-to-target reconciliation and exception reports |
| Cutover | Defines maintenance/read-only expectations for the target | Coordinates export/import sequencing and records its checkpoint |

## New-system obligations

The product work should provide only what is needed for a clean import target:

- stable, versioned import schemas after the relevant domain milestone is
  accepted;
- deterministic external identifiers or an explicit ID-mapping mechanism;
- documented required fields, invariants, scopes, and relationships;
- validation that can run without partially committing a batch;
- idempotency and duplicate-detection behavior;
- clear error codes and field-level diagnostics;
- audit attribution for imported records;
- reconciliation queries or reports;
- a supported way to disable side effects such as notifications, loyalty earns,
  automation, and customer messages during import.

The import mechanism may be a purpose-built API, staged files, or a controlled
staging database. M00/M01 must select the boundary before the migration tool
targets it. Direct writes into live domain tables are not assumed.

## Migration-tool expectations

The separately owned tool should be able to:

- inspect a source without changing it;
- produce a dry-run summary before target mutation;
- preserve a source-to-target ID map;
- resume safely after interruption;
- retry without creating duplicate money, balance, session, or inventory
  records;
- quarantine ambiguous records instead of guessing;
- redact secrets and unnecessary personal data from logs;
- emit counts, totals, skipped records, warnings, and failures by domain;
- verify financial, balance, session, and stock totals after import.

## Explicit non-requirements

- The new APIs and database do not need to accept legacy payloads or table
  shapes.
- The new applications do not need legacy routes, identifiers, or UI behavior.
- The product roadmap does not include implementation or maintenance of the
  migration tool.
- Migration support does not require retaining obsolete domain concepts.

## Planning checkpoints

- M00 defines the import-boundary approach and cross-domain identifier rules.
- M01 publishes organization, location, identity, and Station target schemas.
- Each later domain milestone publishes its import schema only after its model
  is accepted.
- Cutover and reconciliation are planned with the migration-tool owner before a
  venue is moved; they are not backward-compatibility gates for product design.

