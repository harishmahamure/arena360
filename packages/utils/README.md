# @gaming-cafe/utils

Cross-app utilities. The headline export is the shared HTTP client (`createHttpClient`) that wraps axios with envelope unwrapping, JWT/device-token interceptors, retry, and a typed `ApiError`. Validators and small generic helpers live here too.

Currently a skeleton — the real implementation lands in Phase 4. Importers should not rely on any export beyond the placeholder yet.

## Peer dependencies

- `axios@^1.7`
