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

/** Serialize an error for file logging, including stack when available. */
export function serializeError(reason: unknown): string {
  if (reason instanceof Error) {
    const stack = reason.stack ? ` ${reason.stack}` : '';
    return `${reason.name}: ${reason.message}${stack}`;
  }
  return String(reason);
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
  } catch (e) {
    // biome-ignore lint/suspicious/noConsole: non-fatal when diagnostics unavailable
    console.warn('[bootDiagnostics] get_boot_diagnostics failed:', serializeError(e));
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
  } catch (e) {
    // biome-ignore lint/suspicious/noConsole: non-fatal when logging fails
    console.warn('[bootDiagnostics] append_kiosk_log failed:', serializeError(e));
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
    const detail =
      event.error != null ? serializeError(event.error) : event.message || 'Unknown script error';
    addBootError(`JS: ${detail}`);
  });
  window.addEventListener('unhandledrejection', (event) => {
    addBootError(`Unhandled rejection: ${serializeError(event.reason)}`);
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
  } catch (e) {
    // biome-ignore lint/suspicious/noConsole: non-fatal outside Tauri
    console.warn('[bootDiagnostics] kiosk-diagnostic listener failed:', serializeError(e));
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
