# Kiosk games catalog

This folder is the on-device catalog the kiosk renders from. It ships inside the
app bundle (everything under `public/` is served at the web root), so the webview
loads it at runtime from `/games/games.json`.

> Launching is still gated by the client-side allow-list (ADR-0019). The catalog
> only decides what is *shown*; a card is launchable only when its `launchRef`
> resolves to an installed allow-list entry, otherwise it renders a
> "Not installed" badge.

## Layout

```
public/games/
  games.json        # catalog seed (default games/tools)
  gallery.json      # media gallery seed (logos / videos to pick from)
  images/           # thumbnails / logos referenced by the gallery + catalog
  videos/           # hover-preview videos referenced by the gallery + catalog
```

> Both `games.json` and `gallery.json` are **seeds**. Once an admin edits the
> catalog or gallery in Setup mode, the live data is persisted to the kiosk's
> `localStorage` and these files are only used as the first-run defaults.

## `gallery.json` schema

The media gallery the admin picks logos/videos/thumbnails from:

```jsonc
{
  "items": [
    {
      "id": "poster-ember",          // required, unique
      "kind": "image",               // "image" | "video"
      "name": "Ember Poster",        // required, label shown in the picker
      "url": "/games/images/poster-ember.svg" // served path or remote URL
    }
  ]
}
```

## `games.json` schema

```jsonc
{
  "games": [
    {
      "id": "valorant",            // required, stable unique id
      "name": "Valorant",          // required, display name
      "genre": "Tactical Shooter", // optional, shown on the card
      "description": "…",          // optional
      "thumbnailUrl": "/games/images/valorant.png", // optional, poster art (null = icon fallback)
      "logoUrl": null,             // optional
      "videoUrl": "/games/videos/valorant.mp4",     // optional, hover preview
      "icon": "apps",              // optional, Material Symbol for tool tiles
      "subtitle": "Game Launcher", // optional, shown under tool tiles
      "launchRef": "Valorant",     // optional, matches an allow-list entry id or name
      "isActive": true,            // optional, default true; false hides the entry
      "sortOrder": 1,              // optional, ascending; default 0
      "category": "game"           // "game" (Home/Library) | "launcher" | "util" (Settings & Tools)
    }
  ]
}
```

### Notes

- `category: "game"` entries appear on **Home** (Quick Launch) and **Library**.
- `category: "launcher"` / `"util"` entries appear under **Settings & Tools**.
- Asset URLs are served from this folder, e.g. `images/x.png` → `/games/images/x.png`.
  Leave them `null` to render the built-in icon fallback (no broken images).
- `launchRef` is matched case-insensitively against an allow-list entry's id, then
  its name; if omitted, the catalog `name` is matched against entry names.
