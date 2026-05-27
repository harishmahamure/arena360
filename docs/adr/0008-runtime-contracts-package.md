# ADR-0008: Runtime contracts package (@gaming-cafe/contracts)

**Status**: Accepted
**Date**: 2026-05-27
**Deciders**: Platform team (gaming-cafe consolidation working group)

## Context

The consolidation plan includes `@gaming-cafe/api-types` for sharing HTTP
request/response shapes across backend, admin, and kiosk. However, that
package is **generated** by `openapi-typescript` from the backend's
OpenAPI spec — it emits pure TypeScript types with zero runtime footprint.

The backend has **runtime values** that need to be shared:

1. `ErrorCode` enum — used to throw typed errors on the backend and
   display user-friendly messages on admin/kiosk
2. `ErrorMessages` map — the canonical error message for each code
3. `UserRole` type and `UserStatus` enum — used across auth boundaries
4. Pagination types (`IPaginationParams`, `IPaginationResult<T>`) —
   used by backend services and frontend data tables

Today these are defined in `arena360-backend/src/types/*.ts` and
duplicated (often incorrectly) in the kiosk's
`src/shared/types/api.ts` (191 lines of hand-written interfaces) and
admin's `services/**/types.ts` files.

Putting runtime values into `@gaming-cafe/api-types` is impossible:
the package is overwritten on every `pnpm gen:api-types` run. Any
hand-written code there would be lost.

## Decision

We will create a **second shared package**, `@gaming-cafe/contracts`,
for hand-written runtime values and cross-cutting types:

```
packages/
├── api-types/      # Generated HTTP shapes (types only, zero runtime)
└── contracts/      # Hand-written runtime enums, maps, pagination types
```

### Package surface

```typescript
// @gaming-cafe/contracts

// errors.ts
export enum ErrorCode { ... }
export const ErrorMessages: Record<ErrorCode, string> = { ... };
export function isErrorCode(v: string): v is ErrorCode;
export function getErrorMessage(code: ErrorCode): string;

// pagination.ts
export type SortOrder = 'ASC' | 'DESC';
export interface IPaginationParams { page, limit, sortBy?, sortOrder? }
export interface IPaginationResult<T> { data, total, page, limit, totalPages }
export interface IQueryOptions { skip?, take?, order? }
export function calculatePagination(...): ...;
export function toQueryOptions(params: IPaginationParams): IQueryOptions;

// roles.ts
export type UserRole = 'admin' | 'player';
export enum UserStatus { ACTIVE, INACTIVE, SUSPENDED }
export function isUserRole(v: string): v is UserRole;
```

### Consumption

- Backend imports from `@gaming-cafe/contracts` instead of defining
  these locally. The local `src/types/error-codes.types.ts` becomes a
  re-export shim during transition, then is deleted.
- Admin and kiosk import from `@gaming-cafe/contracts` for runtime
  values (enums, error messages) and from `@gaming-cafe/api-types` for
  HTTP shapes (Device, Session, Plan response types).

### Migration task

A new task `move-domain-types` is added to Phase 4 of the consolidation
plan. It:

1. Scaffolds `packages/contracts/` with the files above
2. Codemods backend imports to use `@gaming-cafe/contracts`
3. Deletes kiosk `src/shared/types/api.ts` and admin inline interfaces,
   replacing them with aliases to `@gaming-cafe/api-types`
4. Deletes the backend re-export shims once the codemod is complete

## Consequences

### Positive

- **Single source of truth** for `ErrorCode` — no more drift between
  backend throws and frontend displays.
- **Runtime helpers** (type guards, message lookup) available everywhere
  without copy-paste.
- **Clear separation**: generated vs hand-written, types-only vs runtime.
- **Smaller frontend bundles**: kiosk's 191-line `api.ts` becomes ~30
  lines of aliases.

### Negative

- **One more package** to version and publish. Mitigated by changesets:
  any change to `contracts/` triggers a patch bump automatically.
- **Two imports** for frontend code: `api-types` for shapes,
  `contracts` for enums. Acceptable trade-off for correctness.

### Risks

- **Risk**: Backend defines a new error code but forgets to add it to
  `@gaming-cafe/contracts`.
  **Mitigation**: Backend imports `ErrorCode` from contracts, so any
  new code that throws an undefined code fails TypeScript compilation.
- **Risk**: Drift between contracts and backend over time.
  **Mitigation**: Backend is the sole maintainer of contracts; no other
  workspace adds codes without backend PR approval.

## Alternatives considered

### Put runtime values in @gaming-cafe/api-types

- Pros: Single import for everything
- Cons: `openapi-typescript` regenerates the entire file; any
  hand-written code is lost
- **Why rejected**: Fundamentally incompatible with the codegen workflow

### Duplicate ErrorCode in each app

- Pros: No new package
- Cons: Current pain — kiosk already drifted (missing codes, typos)
- **Why rejected**: This is the problem we're solving

### Inline ErrorCode in backend DTOs so it appears in OpenAPI

- Pros: No new package; codegen picks it up
- Cons: `ErrorCode` is not a DTO; it's a domain constant. Decorating it
  with `@ApiProperty` pollutes the OpenAPI spec and doesn't help
  frontend display logic.
- **Why rejected**: Abuses the OpenAPI contract for non-HTTP concerns

## Implementation notes

- `packages/contracts/package.json` uses `"type": "module"` and exports
  via `exports` map for sub-path imports (`./errors`, `./pagination`,
  `./roles`).
- No build step needed for now — consumers resolve TypeScript sources
  directly via workspace paths. A `tsup` build can be added later if
  we publish to npm.
- The `move-domain-types` task owns the codemod and is sequenced after
  `move-backend` (Phase 2) completes, so paths are stable.

## References

- ADR-0004: Shared API types via OpenAPI (`@gaming-cafe/api-types`)
- `docs/MIGRATION.md` Table 5: backend `src/types/` → `packages/contracts/`
- Consolidation plan, `move-domain-types` task (Phase 4)
