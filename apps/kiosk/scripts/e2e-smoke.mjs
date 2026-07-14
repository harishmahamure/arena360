#!/usr/bin/env node
// biome-ignore-all lint/suspicious/noConsole: CLI smoke-test prints progress to stdout.
/**
 * Kiosk end-to-end smoke test (K8 `kiosk-e2e-smoke`).
 *
 * Scripts the full happy path against a *running* backend:
 *   admin login -> provision device -> player login -> start session -> end session
 * and asserts the single-session guard by starting on a second device.
 *
 * This intentionally talks raw HTTP (no Tauri) so it runs in CI/QA. It is a
 * no-op-with-skip when the required env is absent.
 *
 * Required env:
 *   API_URL                  (default http://localhost:3000)
 *   KIOSK_ADMIN_USERNAME     admin account for provisioning
 *   KIOSK_ADMIN_PASSWORD
 *   KIOSK_PLAYER_USERNAME    a player with a usable balance for these devices
 *   KIOSK_PLAYER_PASSWORD
 * Optional:
 *   KIOSK_ADMIN_TOTP
 */

const API_URL = process.env.API_URL ?? 'http://localhost:3000';

function need(name) {
  const v = process.env[name];
  if (!v) {
    console.log(`SKIP: ${name} not set — e2e smoke requires a seeded backend.`);
    process.exit(0);
  }
  return v;
}

const FINGERPRINT_A = {
  mac: 'AA:BB:CC:DD:EE:01',
  serial: 'E2E-A',
  biosUuid: '00000000-0000-0000-0000-0000000000a1',
  platform: 'windows',
  collectedAt: `${Math.floor(Date.now() / 1000)}`,
};
const FINGERPRINT_B = { ...FINGERPRINT_A, mac: 'AA:BB:CC:DD:EE:02', serial: 'E2E-B' };

async function api(path, { method = 'GET', token, playerToken, body } = {}) {
  const headers = { 'content-type': 'application/json' };
  if (token) headers.authorization = `Bearer ${token}`;
  if (playerToken) headers['x-player-token'] = playerToken;
  const res = await fetch(`${API_URL}${path}`, {
    method,
    headers,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  let json;
  try {
    json = text ? JSON.parse(text) : undefined;
  } catch {
    json = text;
  }
  return { status: res.status, json };
}

function assert(cond, message) {
  if (!cond) {
    console.error(`FAIL: ${message}`);
    process.exit(1);
  }
  console.log(`ok: ${message}`);
}

function unwrap(json, key) {
  return json?.data?.[key] ?? json?.[key];
}

async function adminLogin() {
  const username = need('KIOSK_ADMIN_USERNAME');
  const password = need('KIOSK_ADMIN_PASSWORD');
  const totp = process.env.KIOSK_ADMIN_TOTP;
  const { status, json } = await api('/auth/login/admin', {
    method: 'POST',
    body: { username, password, totp },
  });
  assert(status === 200, `admin login (status ${status})`);
  const token = unwrap(json, 'accessToken');
  assert(Boolean(token), 'received admin accessToken');
  return token;
}

async function provisionDevice(adminToken, name, fingerprint) {
  const { status, json } = await api('/devices/provision', {
    method: 'POST',
    token: adminToken,
    body: {
      fingerprint,
      name,
      deviceType: 'PC',
      deviceSubType: 'HIGH_END_PCS',
      provisionClient: 'kiosk',
    },
  });
  assert(status === 200 || status === 201, `provision ${name} (status ${status})`);
  const deviceToken = unwrap(json, 'accessToken');
  assert(Boolean(deviceToken), `received device token for ${name}`);
  return deviceToken;
}

async function playerLogin(deviceToken, fingerprint) {
  const username = need('KIOSK_PLAYER_USERNAME');
  const password = need('KIOSK_PLAYER_PASSWORD');
  const { status, json } = await api('/auth/login/player', {
    method: 'POST',
    token: deviceToken,
    body: { username, password, fingerprint },
  });
  assert(status === 200, `player login (status ${status})`);
  const playerToken = unwrap(json, 'accessToken');
  assert(Boolean(playerToken), 'received player accessToken');
  return playerToken;
}

async function main() {
  const adminToken = await adminLogin();

  const deviceTokenA = await provisionDevice(adminToken, 'E2E-Device-A', FINGERPRINT_A);
  const deviceTokenB = await provisionDevice(adminToken, 'E2E-Device-B', FINGERPRINT_B);

  const playerTokenA = await playerLogin(deviceTokenA, FINGERPRINT_A);
  const playerTokenB = await playerLogin(deviceTokenB, FINGERPRINT_B);

  const start = await api('/kiosk/sessions', {
    method: 'POST',
    token: deviceTokenA,
    playerToken: playerTokenA,
    body: {},
  });
  assert(start.status === 201, `start session on A (status ${start.status})`);
  const sessionId = unwrap(start.json, 'sessionId');
  assert(Boolean(sessionId), 'received sessionId');

  const conflict = await api('/kiosk/sessions', {
    method: 'POST',
    token: deviceTokenB,
    playerToken: playerTokenB,
    body: {},
  });
  assert(conflict.status === 409, `second device blocked (status ${conflict.status})`);
  const code = conflict.json?.message ?? conflict.json?.data?.message;
  assert(code === 'PLAYER_ALREADY_IN_SESSION', `409 code is PLAYER_ALREADY_IN_SESSION (${code})`);

  const end = await api(`/kiosk/sessions/${sessionId}/end`, {
    method: 'PATCH',
    token: deviceTokenA,
    playerToken: playerTokenA,
    body: { reason: 'voluntary' },
  });
  assert(end.status === 200, `end session on A (status ${end.status})`);

  console.log('\nPASS: kiosk e2e smoke complete.');
}

main().catch((err) => {
  console.error('e2e smoke crashed:', err);
  process.exit(1);
});
