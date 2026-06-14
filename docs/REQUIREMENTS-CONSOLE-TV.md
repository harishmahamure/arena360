# Functional Requirements: Android TV Console Station

## Overview

Kotlin Android TV device agent for **PlayStation stations only** (`PS5`, `PS4`). Staff starts sessions from the admin web app. The TV listens on WebSocket, runs a local interpolated session clock, switches HDMI via CEC, and auto-ends sessions when the local clock reaches zero.

## User Stories (Must)

### US-TV-000 — SSO via WebSocket

**As** staff  
**I want** to send TV login from admin for a PS5/PS4 device  
**So that** the station receives `sso.token.created` and provisions without password entry on the remote

**Given** an unregistered PS5/PS4 device record in admin  
**When** staff clicks “Send TV login”  
**Then** the backend publishes `sso.token.created` on `device:{deviceId}` and the TV auto-redeems

### US-TV-001 — Admin-started session

**Given** a registered console TV with active WebSocket  
**When** staff creates a session in admin for that device  
**Then** the TV enters session phase, anchors its clock from `session.started`, and switches CEC to the console

### US-TV-002 — Auto-end only device HTTP call

**Given** an active session with local clock at zero  
**When** the TV detects expiry  
**Then** it calls `PATCH /tv/sessions/{id}/end` with `reason: auto` and returns to KioskHome on `session.ended`

### US-TV-003 — Admin stop is WS-only

**Given** an active session  
**When** staff ends the session in admin  
**Then** the TV receives `session.ended`, clears session state, returns to KioskHome, and does **not** call PATCH end

### US-TV-004 — Recharge re-anchor

**Given** an active session  
**When** `balance.updated` arrives for the same `sessionId`  
**Then** the local clock re-anchors to `remainingMinutes`

### US-TV-005 — KioskHome looping video

**Given** no active session  
**When** the TV is on KioskHome or waiting for SSO  
**Then** it plays the same looping background video as the Windows kiosk (`launch.webm`)

### US-TV-006 — Audio reminders

**Given** an active session  
**When** the local clock crosses 10, 5, or 2 minutes remaining  
**Then** the TV plays the matching kiosk MP3 assets

### US-TV-007 — CEC auto-discover

**Given** a PlayStation on HDMI  
**When** a session starts  
**Then** the TV discovers the console from the CEC device list and calls `oneTouchPlay` (degraded banner if discovery fails)

### US-TV-008 — PlayStation-only gate

**Given** a `PC` device type  
**When** provisioning or SSO is attempted on Android TV  
**Then** the backend returns `DEVICE_TYPE_NOT_ALLOWED`

## Platform split

| Device type | Client |
|-------------|--------|
| PS5, PS4 | `apps/console-tv` |
| PC | `apps/kiosk` (Tauri) |

## References

- [DRAFT-0035](adr/DRAFT-0035-android-tv-console-station.md)
- [ADR-0018](adr/0018-kiosk-ws-device-acl.md)
