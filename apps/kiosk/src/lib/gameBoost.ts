import { type GameBoostConfig, setGameBoostConfig } from './tauriCommands';

const ENABLED_KEY = 'gaming-cafe.kiosk.game_boost_enabled';
const AGGRESSIVE_KEY = 'gaming-cafe.kiosk.game_boost_aggressive';

export function loadGameBoostSettings(): GameBoostConfig {
  try {
    const enabledRaw = localStorage.getItem(ENABLED_KEY);
    const aggressiveRaw = localStorage.getItem(AGGRESSIVE_KEY);
    return {
      enabled: enabledRaw === null ? true : enabledRaw === 'true',
      aggressive: aggressiveRaw === 'true',
    };
  } catch {
    return { enabled: true, aggressive: false };
  }
}

export function saveGameBoostSettings(config: GameBoostConfig): void {
  try {
    localStorage.setItem(ENABLED_KEY, String(config.enabled));
    localStorage.setItem(AGGRESSIVE_KEY, String(config.aggressive));
  } catch {
    // non-fatal
  }
}

/** Persist to localStorage and sync the native boost module. */
export async function applyGameBoostSettings(config: GameBoostConfig): Promise<void> {
  saveGameBoostSettings(config);
  try {
    await setGameBoostConfig(config);
  } catch {
    // Off-webview dev
  }
}

/** Load from localStorage and push to Rust (setup mount / session start). */
export async function syncGameBoostToNative(): Promise<GameBoostConfig> {
  const config = loadGameBoostSettings();
  try {
    await setGameBoostConfig(config);
  } catch {
    // non-Tauri
  }
  return config;
}
