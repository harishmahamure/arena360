import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';
import type { AppPhase } from '../context/KioskProvider';
import { prepareUpdateRelaunch } from './tauriCommands';

/** Phases where no player session is active and an update check is safe (ADR-0028). */
export type IdleUpdatePhase = Extract<AppPhase, 'register' | 'setup' | 'login'>;

export function isIdleUpdatePhase(phase: AppPhase): phase is IdleUpdatePhase {
  return phase === 'register' || phase === 'setup' || phase === 'login';
}

/**
 * Tauri auto-update manager (ADR-0028).
 *
 * Checks the configured GitHub Releases endpoint for a newer signed build and,
 * if found, downloads, installs, and relaunches. The runtime endpoint + public
 * key are injected only into release builds (see `kiosk-release.yml`); dev and
 * CI builds have no update source, so `check()` simply errors and we no-op.
 *
 * Callers MUST only invoke this while the station is idle (no active player
 * session) — `register`, `setup`, or `login` phases — so an update never
 * interrupts play (ADR-0020 lockdown).
 */
let inFlight = false;

export async function checkForUpdateWhenIdle(): Promise<void> {
  if (inFlight) return;
  inFlight = true;
  try {
    const update = await check();
    if (!update) return;
    await update.downloadAndInstall();
    await prepareUpdateRelaunch();
    await relaunch();
  } catch (err) {
    // Offline, not-yet-configured, or running outside Tauri (browser dev): all
    // non-fatal. The next idle window will retry.
    // biome-ignore lint/suspicious/noConsole: deliberate non-fatal diagnostic for a background update check
    console.warn('[updater] update check skipped:', err);
  } finally {
    inFlight = false;
  }
}
