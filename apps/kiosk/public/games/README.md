# Kiosk media gallery

The kiosk loads a **centrally hosted** media gallery for Setup admins to pick
logos, posters, and preview videos when configuring allowed software.

> Launching is gated by the client-side allow-list (ADR-0019). Each station's
> allow-list decides what is shown and what can launch. Optional media URLs on
> allow-list entries are chosen from this gallery.

## CDN gallery (source of truth)

Hosted at `https://cdn.arena360.cloud/kiosk/gallery.json` by default. Override
per deployment via `VITE_GALLERY_URL`.

Kiosks fetch from the CDN when the Setup gallery picker opens. An internet
connection is required; there is no bundled fallback or on-device cache.

### `gallery.json` schema

```jsonc
{
  "items": [
    {
      "id": "valorant-thumb",          // required, unique
      "kind": "image",                 // "image" | "video"
      "name": "Valorant poster",       // label in the picker
      "url": "https://cdn.arena360.cloud/..." // absolute CDN URL
    }
  ]
}
```

## Allow-list entry media (per station)

When an admin allows software in Setup, they may attach optional media from the
gallery to that entry (stored in localStorage on the station):

| Field | Purpose |
|-------|---------|
| `thumbnailUrl` | Library card / carousel poster |
| `logoUrl` | Hero overlay logo |
| `videoUrl` | Home hero background / card hover preview |
| `genre` | Card genre label (games) |
| `description` | Tool subtitle fallback |
| `icon` | Material Symbol for launcher/util tiles |
| `subtitle` | Settings tool subtitle |
| `sortOrder` | Display order on Home / Library / Tools |

### Video codec notes

Hero / preview videos must use **H.264 in MP4** or **VP9 in WebM**. AV1 often
fails on macOS WKWebView. The app falls back to the login background loop when a
featured video cannot play.
