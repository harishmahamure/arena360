import { invoke } from '@tauri-apps/api/core';

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
