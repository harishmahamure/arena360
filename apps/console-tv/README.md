# Console TV (Android)

Kotlin Android TV agent for PlayStation stations (`PS5`, `PS4`). See [REQUIREMENTS-CONSOLE-TV.md](../../docs/REQUIREMENTS-CONSOLE-TV.md) and [DRAFT-0037](../../docs/adr/DRAFT-0037-console-tv-kiosk-style-provisioning.md).

**IT deployment runbook:** [docs/STATION-DEPLOYMENT-GUIDE.md](../../docs/STATION-DEPLOYMENT-GUIDE.md) (sideload, production API URL, auto-start on boot, lockdown, provisioning, smoke tests).

## Registration

Provisioning matches the PC kiosk flow (see [DRAFT-0023](../../docs/adr/DRAFT-0023-admin-authorized-device-registration.md)):

1. Open the Console TV app on the station (no pre-created device record required).
2. Sign in with an **admin** username and password (enter TOTP if your account requires it).
3. Name the station, choose PS5 or PS4, sub-type, and optional location, then tap **Register device**.

The TV calls `POST /auth/login/admin` then `POST /devices/provision` with `provisionClient: "console-tv"`. After success it connects on the device WebSocket and shows KioskHome until an admin starts a session.

To re-register, clear app data or uninstall/reinstall the app.

## Prerequisites

- JDK 17
- Android SDK (API 35)
- Ruby + Bundler (for Fastlane release builds)

## Local configuration

Copy environment template for API URLs (emulator default uses `10.0.2.2`):

```bash
cp .env.example .env.local
```

## Debug build (unsigned)

```bash
./gradlew testDebugUnitTest assembleDebug
```

## Emulator / touch device (dev)

The manifest declares touch as optional and supports both Android TV (`LEANBACK_LAUNCHER`) and phone/emulator launchers (`LAUNCHER`) for local testing.

From the repo root (requires a running emulator or USB device with `adb`):

```bash
pnpm console-tv:run
```

This uninstalls any existing `com.gamingcafe.consoletv` build, installs a fresh debug APK, and launches `MainActivity`.

To remove a stale install manually:

```bash
adb uninstall com.gamingcafe.consoletv
```

If an old package id remains, list and remove it:

```bash
adb shell pm list packages | grep gamingcafe
adb uninstall <package-name>
```

Emulator API URLs default to `http://10.0.2.2:3000` (host loopback). Ensure the backend is running on your machine and `.env.local` is configured when testing registration or WebSocket flows.

## Release signing

Release builds **require** signing credentials. Unsigned release APK/AAB builds are blocked.

### 1. Generate an upload keystore (once)

```bash
mkdir -p release
keytool -genkeypair -v \
  -keystore release/console-tv.jks \
  -alias console-tv \
  -keyalg RSA -keysize 2048 -validity 10000
```

### 2. Configure local credentials

```bash
cp keystore.properties.example keystore.properties
# Edit keystore.properties with your passwords and storeFile path
```

Alternatively set environment variables:

- `ANDROID_KEYSTORE_PATH` — absolute path to `.jks`
- `ANDROID_KEYSTORE_PASSWORD`
- `ANDROID_KEY_ALIAS`
- `ANDROID_KEY_PASSWORD`

### 3. Build signed release via Fastlane

```bash
bundle install
bundle exec fastlane release
```

Outputs are copied to `fastlane/build/`:

- `*.aab` — Android App Bundle
- `*.apk` — release APK

## CI

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `console-tv-ci.yml` | Push/PR touching `apps/console-tv/**` | Unit tests + debug build (no keystore) |
| `console-tv-release.yml` | Manual (Actions → Run workflow) | Signed AAB + APK → GitHub Release |

### GitHub secrets (release workflow)

| Secret | Description |
|--------|-------------|
| `ANDROID_KEYSTORE_BASE64` | Base64-encoded upload keystore (`.jks`) |
| `ANDROID_KEYSTORE_PASSWORD` | Keystore password |
| `ANDROID_KEY_ALIAS` | Key alias (e.g. `console-tv`) |
| `ANDROID_KEY_PASSWORD` | Key password |

Encode keystore for CI:

```bash
base64 -i release/console-tv.jks | pbcopy
```

**Never commit** `keystore.properties`, `*.jks`, or `release/` contents.

## Play Store

Play Store upload (`supply`) is not configured yet. Releases are distributed as signed artifacts attached to GitHub Releases.
