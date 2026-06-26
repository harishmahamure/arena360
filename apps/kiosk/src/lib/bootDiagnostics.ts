import { invoke } from '@tauri-apps/api/core';

export interface BootDiagnosticsSnapshot {
  logPath: string;
  recentLines: string[];
  errors: string[];
}

type Listener = (snap: BootDiagnosticsSnapshot) => void;

const listeners = new Set<Listener>();
let cached: BootDiagnosticsSnapshot = { logPath: '', recentLines: [], errors: [] };

function isTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

function notify() {
  for (const fn of listeners) fn(cached);
}

export function subscribeBootErrors(fn: Listener): () => void {
  listeners.add(fn);
  fn(cached);
  return () => listeners.delete(fn);
}

export async function getBootDiagnosticsSnapshot(): Promise<BootDiagnosticsSnapshot> {
  if (!isTauri()) return cached;
  try {
    cached = await invoke<BootDiagnosticsSnapshot>('get_boot_diagnostics');
    notify();
    return cached;
  } catch {
    return cached;
  }
}

export async function appendKioskLog(level: string, message: string): Promise<void> {
  if (!isTauri()) {
    // biome-ignore lint/suspicious/noConsole: non-Tauri dev/test fallback
    console.warn(`[kiosk:${level}]`, message);
    if (level.toUpperCase() === 'ERROR') {
      cached = {
        ...cached,
        errors: [...cached.errors, message].slice(-50),
      };
      notify();
    }
    return;
  }
  try {
    await invoke('append_kiosk_log', { level, message });
    await getBootDiagnosticsSnapshot();
  } catch {
    // Non-fatal when logging fails.
  }
}

export function addBootError(message: string) {
  cached = {
    ...cached,
    errors: [...cached.errors, message].slice(-50),
  };
  notify();
  void appendKioskLog('error', message);
}

export async function initBootDiagnostics(): Promise<void> {
  window.addEventListener('error', (event) => {
    const msg = event.message || 'Unknown script error';
    addBootError(`JS: ${msg}`);
  });
  window.addEventListener('unhandledrejection', (event) => {
    const reason = event.reason instanceof Error ? event.reason.message : String(event.reason);
    addBootError(`Unhandled rejection: ${reason}`);
  });

  if (!isTauri()) return;

  try {
    const { listen } = await import('@tauri-apps/api/event');
    await listen<{ level?: string; message?: string }>('kiosk-diagnostic', (event) => {
      const message = event.payload.message ?? 'Unknown diagnostic event';
      if (event.payload.level?.toUpperCase() === 'ERROR') {
        addBootError(message);
      } else {
        void appendKioskLog(event.payload.level ?? 'info', message);
      }
    });
  } catch {
    // Non-fatal outside Tauri.
  }

  await getBootDiagnosticsSnapshot();
}

export async function reportBootReady(phase = 'mounted'): Promise<void> {
  await appendKioskLog('info', `react boot_ready phase=${phase}`);
}

export function verifyStylesLoaded(): void {
  requestAnimationFrame(() => {
    const bg = getComputedStyle(document.documentElement).getPropertyValue('--kiosk-bg').trim();
    if (!bg) {
      addBootError('Stylesheet failed to load (--kiosk-bg missing)');
    }
  });
}
