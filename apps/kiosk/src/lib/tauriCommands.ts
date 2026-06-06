import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';

export interface TokenStore {
  deviceToken?: string;
  playerToken?: string;
}

export interface FingerprintPayload {
  mac: string;
  serial: string;
  biosUuid: string;
  platform: string;
  collectedAt: string;
}

export async function getTokens(): Promise<TokenStore> {
  return invoke<TokenStore>('get_tokens');
}

export async function setDeviceToken(token: string): Promise<void> {
  await invoke('set_device_token', { token });
}

export async function setPlayerToken(token: string): Promise<void> {
  await invoke('set_player_token', { token });
}

export async function clearPlayerToken(): Promise<void> {
  await invoke('clear_player_token');
}

export async function clearAllTokens(): Promise<void> {
  await invoke('clear_all_tokens');
}

export async function collectFingerprint(): Promise<FingerprintPayload> {
  return invoke<FingerprintPayload>('collect_fingerprint');
}

export async function setLockdownState(state: 'Locked' | 'SetupRelaxed'): Promise<void> {
  await invoke('set_lockdown_state', { state });
}

export type LockdownState = 'Locked' | 'SetupRelaxed';

export async function getLockdownState(): Promise<LockdownState> {
  return invoke<LockdownState>('get_lockdown_state');
}

export async function getSystemVolume(): Promise<number> {
  return invoke<number>('get_system_volume');
}

export async function setSystemVolume(volume: number): Promise<number> {
  return invoke<number>('set_system_volume', { volume });
}

export async function openAudioSettings(): Promise<void> {
  await invoke('open_audio_settings');
}

export async function lockWorkstation(): Promise<void> {
  await invoke('lock_workstation');
}

export async function restartStation(): Promise<void> {
  await invoke('restart_station');
}

export async function shutdownStation(): Promise<void> {
  await invoke('shutdown_station');
}

export interface ScanCandidate {
  name: string;
  executablePath: string;
  source: string;
  present: boolean;
}

export async function scanInstalledSoftware(): Promise<ScanCandidate[]> {
  return invoke<ScanCandidate[]>('scan_installed_software');
}

/**
 * Open the native OS file picker so an admin can browse for an executable to
 * add to the allow-list. Returns the absolute path, or `null` when the user
 * cancels or the dialog is unavailable (e.g. plain browser dev).
 */
export async function pickExecutable(): Promise<string | null> {
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      title: 'Select an application',
      filters: [
        { name: 'Programs', extensions: ['exe', 'bat', 'cmd', 'com', 'lnk'] },
        { name: 'All files', extensions: ['*'] },
      ],
    });
    return typeof selected === 'string' ? selected : null;
  } catch {
    return null;
  }
}

/**
 * Open the native file picker so an admin can browse for a gallery asset
 * (image or video). Returns a webview-renderable source via `convertFileSrc`,
 * or `null` when cancelled / unavailable (e.g. plain browser dev).
 */
export async function pickMediaFile(kind: 'image' | 'video'): Promise<string | null> {
  const filters =
    kind === 'video'
      ? [{ name: 'Videos', extensions: ['mp4', 'webm', 'mov', 'm4v', 'ogv'] }]
      : [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp', 'svg', 'avif'] }];
  try {
    const selected = await open({
      multiple: false,
      directory: false,
      title: kind === 'video' ? 'Select a video' : 'Select an image',
      filters,
    });
    return typeof selected === 'string' ? convertFileSrc(selected) : null;
  } catch {
    return null;
  }
}

export interface TrackedProcess {
  pid: number;
  executablePath: string;
}

export async function launchAllowed(
  executablePath: string,
  allowList: string[],
  args?: string,
): Promise<{ pid: number }> {
  return invoke('launch_allowed', {
    executablePath,
    arguments: args,
    allowList,
  });
}

export async function getTrackedProcesses(): Promise<TrackedProcess[]> {
  return invoke<TrackedProcess[]>('get_tracked_processes');
}

export async function killTrackedProcesses(graceSeconds?: number): Promise<{ killed: number }> {
  return invoke('kill_tracked_processes', { graceSeconds });
}

export async function clearTrackedProcesses(): Promise<void> {
  await invoke('clear_tracked_processes');
}

/** Read cached CDN gallery from app data (`gallery_cache.json` shape). */
export async function readGalleryCacheFs(): Promise<{ items: unknown[] } | null> {
  try {
    const raw = await invoke<{ items?: unknown[] } | null>('read_gallery_cache');
    if (!raw || !Array.isArray(raw.items)) return null;
    return { items: raw.items };
  } catch {
    return null;
  }
}

/** Persist a CDN gallery fetch to app data for offline picker use. */
export async function writeGalleryCacheFs(items: unknown[]): Promise<void> {
  try {
    await invoke('write_gallery_cache', { items });
  } catch {
    // Non-Tauri dev (browser) — no FS cache.
  }
}

/**
 * Download a remote asset into the on-device cache (once) and return a source
 * the webview can render. Falls back to the remote URL when not running under
 * Tauri (e.g. browser dev) or when caching fails (DRAFT-0022).
 */
export async function cachedAssetSrc(url: string): Promise<string> {
  if (!url) return url;
  try {
    const path = await invoke<string>('cache_asset', { url });
    return convertFileSrc(path);
  } catch {
    return url;
  }
}
