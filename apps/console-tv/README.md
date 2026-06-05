# Console TV (`@gaming-cafe/console-tv`)

React Native Android TV overlay for PlayStation stations. Staff start and end sessions from the admin counter; this app shows a time HUD on the TV with **no player login**.

## Requirements

- Node 20+
- pnpm 9+
- Android SDK (API 34+) for builds
- Android TV emulator, physical Android TV, or Amazon Fire TV stick (sideload)

## Setup

```bash
# From repo root
pnpm install
```

Set the API URL in [`src/lib/config.ts`](src/lib/config.ts) (default `http://10.0.2.2:3000` for the Android emulator → host machine). See [`.env.example`](.env.example).

## Run

```bash
pnpm console-tv:start          # Metro bundler
pnpm console-tv:android        # Install debug APK on connected device/emulator
```

## First boot (provisioning)

1. Launch the app on the TV.
2. Sign in with an **administrator** account (username/password → OTP).
3. Name the station and choose device type (`PS5`, `PS4`, or `CONSOLE`) and console tier.
4. The app calls existing `POST /devices/provision` and stores the device JWT.

## Runtime behaviour

- Subscribes to WebSocket channel `device:{id}` with the device token.
- On `session.started`: show overlay; if `remainingMinutes` is in the payload, run a local countdown ([`HudTimer`](src/components/HudTimer.tsx)).
- On `session.ended`: hide overlay.
- On `balance.updated` (mid-session recharge): re-anchor remaining time.
- Audio reminders at 10 / 5 / 2 minutes via native beeps.

### WebSocket `remainingMinutes` prerequisite

Committed backend sends `sessionId` and `deviceId` on `session.started` to the device channel. **Accurate countdown digits require `remainingMinutes` (and ideally `startTime`) in that event payload.** Without it, the app shows a “Session active” badge until `session.ended`. Enriching the WebSocket payload is a separate backend change if needed.

## Fire TV sideload

```bash
cd apps/console-tv/android
./gradlew assembleRelease
adb connect <fire-ip>:5555
adb install app/build/outputs/apk/release/app-release.apk
```

Use the same APK for Android TV leanback launchers.

## Architecture

| Phase | Screen |
|-------|--------|
| `setup` | Admin OTP + device provisioning |
| `idle` | Waiting for staff-started session |
| `overlay` | HUD timer on top of idle |

No REST session polling — WebSocket + local tick only (DRAFT-0029).

## Tests

```bash
pnpm --filter @gaming-cafe/console-tv test
```
