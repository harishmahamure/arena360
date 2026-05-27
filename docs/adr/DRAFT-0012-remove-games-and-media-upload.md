# DRAFT-0012: Remove games, device games, and media upload

**Status**: Proposed
**Date**: 2026-05-28
**Deciders**: Platform team

## Context

The gaming cafe management system originally included per-game inventory tracking (`games` table), device-game assignments (`device_games` junction table), and a general-purpose media upload system (`files` table + Cloudflare R2 storage integration). After operational experience:

1. **Games** — the cafe does not need to track individual game titles. Devices are booked by type (PC, PS5, etc.), not by game.
2. **Device games** — junction table served no operational purpose. Session creation never required selecting a game in the UI.
3. **Media upload** — file upload for game images/videos was never used in production. Game media fields stored URL strings, not file records.

These features add maintenance burden (6 backend modules, 9 frontend pages/services, R2 cloud dependency) with no business value.

## Decision

Remove all three features entirely:

- Drop `games`, `device_games`, and `files` database tables
- Remove backend handlers, services, repositories, and models
- Remove frontend pages, services, containers, and navigation items
- Remove `GamesRead/Write`, `DeviceGamesRead/Write`, `FilesRead/Write` permissions
- Remove R2/S3 storage integration and configuration
- Drop `usage_sessions."gameId"` column (optional FK to games)

## Consequences

### Positive
- Reduced codebase (~30 files removed)
- Eliminated R2 cloud dependency and credentials
- Simplified navigation for staff/admin users
- Faster builds (fewer Rust modules, fewer AWS SDK deps)

### Negative
- Any existing `games`, `device_games`, `files` data will be permanently deleted
- URL strings in game `imageUrl`/`videoUrl` columns become inaccessible
- R2 bucket objects are NOT deleted (manual cleanup required)

### Risks
- **Risk**: Other tables may FK to games. **Mitigation**: Only `usage_sessions."gameId"` and `device_games."gameId"` reference games; both are dropped in the migration.

## Alternatives Considered

### Keep tables, remove UI only
- Pros: Data preserved for future use
- Cons: Dead schema, continued migration maintenance
- **Why rejected**: Tables serve no purpose; schema should match features

## References
- ADR-0009: Rust Axum backend
- ADR-0011: POS shifts, registers, expenses
