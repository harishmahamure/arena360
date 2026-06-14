#!/usr/bin/env node
/**
 * Copies the built arena360-watchdog binary to the path Tauri externalBin expects
 * (Windows releases only). No-op on other platforms.
 */
import { execSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const kioskRoot = path.resolve(__dirname, '..');
const tauriDir = path.join(kioskRoot, 'src-tauri');

if (process.platform !== 'win32') {
  process.exit(0);
}

const triple = execSync('rustc --print host-tuple', { encoding: 'utf8' }).trim();
const manifest = path.join(tauriDir, 'Cargo.toml');
const releaseBin = path.join(tauriDir, 'target', 'release', 'arena360-watchdog.exe');
const sidecarName = `arena360-watchdog-${triple}.exe`;
const sidecarPath = path.join(tauriDir, sidecarName);

execSync(`cargo build --release --manifest-path "${manifest}" --bin arena360-watchdog`, {
  stdio: 'inherit',
  cwd: kioskRoot,
  env: { ...process.env, SKIP_TAURI_EXTERNAL_BINS: '1' },
});

if (!fs.existsSync(releaseBin)) {
  console.error(`Watchdog binary missing after build: ${releaseBin}`);
  process.exit(1);
}

fs.copyFileSync(releaseBin, sidecarPath);
console.log(`Prepared watchdog sidecar: ${sidecarPath}`);
