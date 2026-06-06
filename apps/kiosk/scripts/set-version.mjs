#!/usr/bin/env node
/**
 * Kiosk version fan-out (ADR-0028).
 *
 * Bumps the single source of truth across the three files that must stay in
 * sync, so the version can never drift:
 *   - src-tauri/tauri.conf.json  ("version")
 *   - src-tauri/Cargo.toml       ([package] version)
 *   - package.json               ("version")
 *
 * Usage:
 *   node scripts/set-version.mjs <patch|minor|major>
 *   node scripts/set-version.mjs 1.4.0           # explicit semver
 *
 * Prints `version=X.Y.Z` to stdout for GitHub Actions ($GITHUB_OUTPUT).
 */
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const kioskDir = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const tauriConfPath = resolve(kioskDir, 'src-tauri/tauri.conf.json');
const cargoPath = resolve(kioskDir, 'src-tauri/Cargo.toml');
const packagePath = resolve(kioskDir, 'package.json');

const SEMVER = /^\d+\.\d+\.\d+$/;

function nextVersion(current, arg) {
  if (SEMVER.test(arg)) return arg;
  const [major, minor, patch] = current.split('.').map((n) => Number.parseInt(n, 10));
  if (![major, minor, patch].every(Number.isFinite)) {
    throw new Error(`Cannot parse current version "${current}"`);
  }
  switch (arg) {
    case 'major':
      return `${major + 1}.0.0`;
    case 'minor':
      return `${major}.${minor + 1}.0`;
    case 'patch':
      return `${major}.${minor}.${patch + 1}`;
    default:
      throw new Error(`Expected "patch" | "minor" | "major" | X.Y.Z, got "${arg}"`);
  }
}

function writeJsonVersion(path, version) {
  const src = readFileSync(path, 'utf8');
  // Replace only the top-level "version" field so Biome formatting is preserved
  // (JSON.stringify would re-expand short arrays and fail the pre-commit hook).
  const next = src.replace(/^  "version": "\d+\.\d+\.\d+",$/m, `  "version": "${version}",`);
  if (next === src) {
    throw new Error(`No top-level "version" field replaced in ${path}`);
  }
  writeFileSync(path, next);
}

function writeCargoVersion(path, version) {
  const src = readFileSync(path, 'utf8');
  // Only the [package] table has a line-anchored `version = "..."`; dependency
  // versions are inline (`{ version = "2" }`) and are left untouched.
  const next = src.replace(/^version = "\d+\.\d+\.\d+"$/m, `version = "${version}"`);
  if (next === src) {
    throw new Error(`No [package] version line replaced in ${path}`);
  }
  writeFileSync(path, next);
}

const bumpArg = process.argv[2];
if (!bumpArg) {
  process.stderr.write('Missing argument: <patch|minor|major> or an explicit X.Y.Z\n');
  process.exit(1);
}

const current = JSON.parse(readFileSync(tauriConfPath, 'utf8')).version;
const version = nextVersion(current, bumpArg);

writeJsonVersion(tauriConfPath, version);
writeJsonVersion(packagePath, version);
writeCargoVersion(cargoPath, version);

process.stderr.write(`kiosk version: ${current} -> ${version}\n`);
process.stdout.write(`version=${version}\n`);
