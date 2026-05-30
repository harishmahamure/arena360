/**
 * @gaming-cafe/contracts
 *
 * Runtime enums, pagination types, and role contracts shared across
 * all gaming-cafe workspaces (backend, admin, kiosk).
 *
 * This package contains hand-written types that need to be shared at runtime,
 * as opposed to @gaming-cafe/api-types which contains generated HTTP shape types
 * from the OpenAPI spec.
 *
 * @see ADR-0008 for rationale on the two-package split
 */

export * from './enums.js';
export * from './errors.js';
export * from './pagination.js';
export * from './roles.js';
