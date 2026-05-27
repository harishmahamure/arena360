# @gaming-cafe/contracts

Runtime enums, pagination types, and role contracts shared across all gaming-cafe workspaces.

## What this package is

This package contains **hand-written types that need to be shared at runtime**, including:

- **Error codes** (`ErrorCode` enum + `ErrorMessages` map) — used by backend to throw errors and by admin/kiosk to display user-friendly messages
- **Pagination types** (`IPaginationParams`, `IPaginationResult<T>`, etc.) — used by backend services and frontend data tables
- **Role types** (`UserRole`, `UserStatus`) — used across auth boundaries

## What this package is NOT

- **Generated HTTP types** — those live in `@gaming-cafe/api-types` (generated from OpenAPI spec via `openapi-typescript`)
- **UI components** — those live in `@gaming-cafe/ui`
- **Backend-only types** — DTOs with Nest decorators, TypeORM entities, Fastify augmentations stay in `apps/backend`

## Usage

```typescript
// Import everything
import { ErrorCode, ErrorMessages, UserRole, IPaginationResult } from '@gaming-cafe/contracts';

// Or import specific modules
import { ErrorCode, getErrorMessage } from '@gaming-cafe/contracts/errors';
import { IPaginationParams, calculatePagination } from '@gaming-cafe/contracts/pagination';
import { UserRole, isUserRole } from '@gaming-cafe/contracts/roles';
```

## When to extend this package

Add to this package when:

1. You have a **runtime value** (enum, constant map) that needs to be shared between backend and frontend
2. You have a **cross-cutting type** that doesn't belong to any single domain module
3. The type is **not generated** from the OpenAPI spec

Do NOT add:

- Types that are only used in one workspace
- Generated types (use `pnpm gen:api-types` instead)
- Types with framework-specific decorators (those stay in `apps/backend`)

## Related

- `@gaming-cafe/api-types` — generated HTTP shape types from OpenAPI spec
- `ADR-0008` — rationale for the two-package split
- `ADR-0004` — shared API types via OpenAPI
