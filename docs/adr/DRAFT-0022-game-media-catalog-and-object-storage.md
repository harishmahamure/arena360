# DRAFT-0022: Game media catalog and object storage

**Status**: Proposed
**Date**: 2026-05-30
**Deciders**: Platform team
**Supersedes**: DRAFT-0012 (media-upload removal portion)

## Context

The kiosk is being redesigned to mirror the ggLeap client: a logged-out login
screen with a center logo and a background (image or looped video), and a
logged-in **game-card grid** with per-game thumbnails/logos. None of this content
exists in the database today — the `games`, `device_games`, and `files` tables
were dropped in migration `20260528000001` and DRAFT-0012 proposed removing media
upload entirely.

We need a place to store, per game, a **thumbnail**, a **logo**, and a **video**
(used as a background and cached on-device for smooth playback). Admins add these
assets from the admin app. The launch allow-list stays client-side per ADR-0019;
this catalog is **display-only**.

## Decision

1. **Re-introduce a `games` table** (display-only catalog), distinct from the old
   inventory table. Columns: `id`, `name`, `thumbnailUrl`, `logoUrl`, `videoUrl`,
   `launchRef` (nullable; matches a client allow-list entry by name/id),
   `isActive`, `sortOrder`, plus audit + soft-delete columns following the
   existing TypeORM camelCase quoting convention.
2. **Object storage** for the binary assets. The DB stores **URLs/keys only**;
   files live in an S3-compatible bucket (S3 / MinIO). The backend issues
   **SigV4 presigned PUT URLs** so the admin uploads directly to storage, then
   persists the returned public URL on the game.
3. **HTTP surface** (REST, per `04-api-design.mdc`):
   - `GET /games` — active catalog (kiosk + admin)
   - `POST /games`, `PATCH /games/{id}`, `DELETE /games/{id}` (admin)
   - `POST /uploads/presign` (admin) → `{ uploadUrl, publicUrl, key }`
4. **Relationship to ADR-0019**: unchanged. Launching still resolves through the
   client-side allow-list; `games.launchRef` is an optional display→launch hint.
   The server is **not** the launch source of truth.
5. **New crates** (per `11-rust.mdc`): `hmac`, `sha2`, `hex` for SigV4 presigning.
   No heavyweight AWS SDK is added.
6. **Device caching**: the kiosk downloads the `videoUrl` via a Tauri command into
   the app cache dir and renders the local file, falling back to the remote URL.

## Consequences

### Positive
- Branded ggLeap-style kiosk backed entirely by DB/storage data.
- Direct-to-storage uploads keep large media off the API process.
- Minimal crate footprint (no AWS SDK).

### Negative
- Re-introduces a `games` concept after DRAFT-0012 proposed its removal.
- Adds object-storage configuration/secrets and an `infra/**` dependency.

### Risks
- **Stale cached video** on devices. **Mitigation**: cache key derived from the
  asset URL; changing the URL invalidates the cache.
- **Presign misconfiguration**. **Mitigation**: unit-test SigV4 against known
  vectors; presigned URLs are short-lived.

## Alternatives Considered

### Store bytes in Postgres (bytea)
- Pros: no extra infra. Cons: bloats DB, poor for video streaming. Rejected.

### Backend-served static uploads dir
- Pros: simplest. Cons: couples media lifecycle to API pods, no CDN path. Rejected
  in favor of object storage (chosen by product).

### Games table also drives launch (supersede ADR-0019)
- Rejected: keep launch client-side (ADR-0019); catalog stays display-only.

## References
- ADR-0009 Rust Axum backend
- ADR-0019 Kiosk device allow-list (client-side)
- DRAFT-0012 Remove games and media upload (superseded in part)
