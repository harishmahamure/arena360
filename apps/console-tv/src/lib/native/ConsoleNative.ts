import { NativeModules, Platform } from 'react-native';

export interface FingerprintPayload {
  mac: string;
  serial: string;
  biosUuid: string;
  platform: string;
  collectedAt: string;
  androidId?: string;
  manufacturer?: string;
  model?: string;
}

interface ConsoleNativeModule {
  getFingerprint(): Promise<FingerprintPayload>;
  playReminder(which: 'two' | 'five' | 'ten'): void;
  setKeepScreenOn(enabled: boolean): void;
}

const native: ConsoleNativeModule | undefined =
  Platform.OS === 'android' ? NativeModules.ConsoleNative : undefined;

export async function collectFingerprint(): Promise<FingerprintPayload> {
  if (!native?.getFingerprint) {
    return {
      mac: 'N/A',
      serial: 'N/A',
      biosUuid: 'N/A',
      platform: 'android_tv',
      collectedAt: new Date().toISOString(),
    };
  }
  return native.getFingerprint();
}

export function playReminder(which: 'two' | 'five' | 'ten'): void {
  native?.playReminder?.(which);
}

export function setKeepScreenOn(enabled: boolean): void {
  native?.setKeepScreenOn?.(enabled);
}
