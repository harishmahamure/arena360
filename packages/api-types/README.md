# @gaming-cafe/api-types

Generated TypeScript types for the backend REST API. Driven end-to-end by `openapi-typescript`:

1. `apps/backend/scripts/generate-openapi.ts` (built in a later phase) emits `apps/backend/docs/openapi.json` from the live Nest application.
2. The root script `scripts/gen-api-types.ts` pipes that JSON through `openapi-typescript` into `src/schema.ts` here.
3. Consumers import via `@gaming-cafe/api-types` (`paths`, `components`, `operations`).

## Regenerating

```bash
pnpm gen:api-types
```

CI runs the same command and fails if `src/schema.ts` drifts from what the spec produces (see ADR-0004). **Never hand-edit `src/schema.ts`.**
