import { useCallback, useEffect, useMemo, useState } from 'react';
import type { LaunchEntry } from '../../lib/allowList';
import { loadLaunchEntries } from '../../lib/allowList';
import { installedEntries } from '../../lib/launch';
import {
  getTrackedProcesses,
  killTrackedProcesses,
  type TrackedProcess,
} from '../../lib/tauriCommands';

export interface TrackedApp {
  pid: number;
  executablePath: string;
  displayName: string;
  /** True when the tracked root is a platform launcher awaiting player sign-in. */
  awaitingLauncherLogin: boolean;
}

const POLL_MS = 2000;

function normalizePath(path: string): string {
  return path.replace(/\\/g, '/').toLowerCase();
}

function displayNameForPath(path: string, entries: LaunchEntry[]): string {
  const norm = normalizePath(path);
  const viaMatch = entries.find(
    (entry) => entry.launchVia && norm.endsWith(normalizePath(entry.launchVia.executablePath)),
  );
  if (viaMatch) return viaMatch.name;
  const match = entries.find((entry) => norm.endsWith(normalizePath(entry.executablePath)));
  if (match) return match.name;
  const base = path.split(/[/\\]/).pop() ?? path;
  return base.replace(/\.exe$/i, '');
}

function isLauncherTrackedPath(path: string): boolean {
  const base = (path.split(/[/\\]/).pop() ?? '').toLowerCase();
  const norm = path.replace(/\\/g, '/').toLowerCase();
  return (
    base === 'riotclientservices.exe' ||
    base === 'steam.exe' ||
    base === 'epicgameslauncher.exe' ||
    base === 'battle.net launcher.exe' ||
    base === 'battle.net.exe' ||
    base === 'ubisoftconnect.exe' ||
    base === 'ealauncher.exe' ||
    base === 'ealaunchhelper.exe' ||
    base === 'galaxyclient.exe' ||
    (base === 'launcher.exe' && norm.includes('/rockstar games/launcher/'))
  );
}

function toTrackedApps(processes: TrackedProcess[], entries: LaunchEntry[]): TrackedApp[] {
  return processes.map((process) => ({
    pid: process.pid,
    executablePath: process.executablePath,
    displayName: displayNameForPath(process.executablePath, entries),
    awaitingLauncherLogin: isLauncherTrackedPath(process.executablePath),
  }));
}

interface UseTrackedProcessesOptions {
  enabled: boolean;
  onError?: (message: string) => void;
}

/** Poll native tracked PIDs during an active player session (ADR-0020). */
export function useTrackedProcesses({ enabled, onError }: UseTrackedProcessesOptions) {
  const entries = useMemo(() => installedEntries(), []);
  const [processes, setProcesses] = useState<TrackedApp[]>([]);
  const [closing, setClosing] = useState(false);

  const refresh = useCallback(async () => {
    if (!enabled) {
      setProcesses([]);
      return;
    }
    try {
      const tracked = await getTrackedProcesses();
      setProcesses(toTrackedApps(tracked, entries.length > 0 ? entries : loadLaunchEntries()));
    } catch {
      // Off-webview dev or non-Tauri: treat as empty.
      setProcesses([]);
    }
  }, [enabled, entries]);

  useEffect(() => {
    if (!enabled) {
      setProcesses([]);
      return;
    }
    void refresh();
    const id = setInterval(() => {
      void refresh();
    }, POLL_MS);
    return () => clearInterval(id);
  }, [enabled, refresh]);

  useEffect(() => {
    if (!enabled) return;
    const onFocus = () => {
      void refresh();
    };
    window.addEventListener('focus', onFocus);
    return () => window.removeEventListener('focus', onFocus);
  }, [enabled, refresh]);

  const closeAll = useCallback(async () => {
    if (closing || processes.length === 0) return;
    setClosing(true);
    try {
      await killTrackedProcesses(0);
      setProcesses([]);
    } catch (e) {
      onError?.(e instanceof Error ? e.message : 'Could not close the running app');
    } finally {
      setClosing(false);
    }
  }, [closing, onError, processes.length]);

  return { processes, closing, closeAll, refresh };
}
