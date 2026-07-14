import { invoke } from '@tauri-apps/api/core';
import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';
import type { AppPhase } from '../context/KioskProvider';
import { appendKioskLog, serializeError } from './bootDiagnostics';

/** Phases where no player session is active and an update check is safe (ADR-0028). */
export type IdleUpdatePhase = Extract<
  AppPhase,
  'register' | 'setup' | 'login' | 'create-account' | 'create-account-success'
>;

export const IDLE_UPDATE_RECHECK_MS = 30 * 60 * 1000;

export function isIdleUpdatePhase(phase: AppPhase): phase is IdleUpdatePhase {
  return (
    phase === 'register' ||
    phase === 'setup' ||
    phase === 'login' ||
    phase === 'create-account' ||
    phase === 'create-account-success'
  );
}

/**
 * Tauri auto-update manager (ADR-0028, ADR-0048).
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
let cancelRequested = false;
let trackedPhase: AppPhase | null = null;
let idleRecheckTimer: ReturnType<typeof setInterval> | null = null;

/** Keep phase in sync so an in-progress download can abort if play starts. */
export function setUpdatePhase(phase: AppPhase): void {
  trackedPhase = phase;
  if (!isIdleUpdatePhase(phase)) {
    cancelRequested = true;
    stopIdleRecheckTimer();
    return;
  }
  startIdleRecheckTimer(phase);
}

function shouldContinueUpdate(phase: AppPhase): boolean {
  return !cancelRequested && isIdleUpdatePhase(trackedPhase ?? phase);
}

function stopIdleRecheckTimer(): void {
  if (idleRecheckTimer) {
    clearInterval(idleRecheckTimer);
    idleRecheckTimer = null;
  }
}

function startIdleRecheckTimer(phase: IdleUpdatePhase): void {
  stopIdleRecheckTimer();
  idleRecheckTimer = setInterval(() => {
    if (isIdleUpdatePhase(trackedPhase ?? phase)) {
      void checkForUpdateWhenIdle(trackedPhase ?? phase);
    }
  }, IDLE_UPDATE_RECHECK_MS);
}

export async function checkForUpdateWhenIdle(phase: AppPhase): Promise<void> {
  if (inFlight) return;
  if (!isIdleUpdatePhase(phase)) return;

  inFlight = true;
  cancelRequested = false;
  trackedPhase = phase;

  let installCompleted = false;

  try {
    const update = await check();
    if (!update) return;
    if (!shouldContinueUpdate(phase)) return;

    await invoke('prepare_for_update');
    if (!shouldContinueUpdate(phase)) return;

    await update.downloadAndInstall();
    installCompleted = true;

    if (shouldContinueUpdate(phase)) {
      await relaunch();
      return;
    }

    // Install already applied — relaunch even if phase left idle mid-handoff.
    await relaunch();
  } catch (err) {
    if (installCompleted) {
      try {
        await relaunch();
      } catch (relaunchErr) {
        await appendKioskLog(
          'error',
          `[updater] relaunch after install failed: ${serializeError(relaunchErr)}`,
        );
      }
      return;
    }
    // Offline, not-yet-configured, or running outside Tauri (browser dev): all
    // non-fatal. The next idle window will retry.
    await appendKioskLog('warn', `[updater] update check skipped: ${serializeError(err)}`);
  } finally {
    inFlight = false;
  }
}
