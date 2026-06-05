import AsyncStorage from '@react-native-async-storage/async-storage';

const DEVICE_TOKEN_KEY = 'gaming-cafe.console-tv.device_token';
const DEVICE_NAME_KEY = 'gaming-cafe.console-tv.device_name';

export async function loadDeviceToken(): Promise<string | null> {
  try {
    return await AsyncStorage.getItem(DEVICE_TOKEN_KEY);
  } catch {
    return null;
  }
}

export async function persistDeviceToken(token: string): Promise<void> {
  await AsyncStorage.setItem(DEVICE_TOKEN_KEY, token);
}

export async function clearDeviceToken(): Promise<void> {
  await AsyncStorage.removeItem(DEVICE_TOKEN_KEY);
}

export async function loadDeviceName(): Promise<string | null> {
  try {
    return await AsyncStorage.getItem(DEVICE_NAME_KEY);
  } catch {
    return null;
  }
}

export async function persistDeviceName(name: string): Promise<void> {
  await AsyncStorage.setItem(DEVICE_NAME_KEY, name);
}

export function deviceIdFromToken(token: string): string | null {
  try {
    const payload = JSON.parse(atob(token.split('.')[1] ?? ''));
    return (payload.userId as string) ?? null;
  } catch {
    return null;
  }
}
