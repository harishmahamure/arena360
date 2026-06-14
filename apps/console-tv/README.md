# Console TV (Android)

Kotlin Android TV agent for PlayStation stations (`PS5`, `PS4`). See [REQUIREMENTS-CONSOLE-TV.md](../../docs/REQUIREMENTS-CONSOLE-TV.md) and [DRAFT-0035](../../docs/adr/DRAFT-0035-android-tv-console-station.md).

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
