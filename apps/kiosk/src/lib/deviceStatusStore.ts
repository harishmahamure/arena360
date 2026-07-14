const DEVICE_STATUS_KEY = 'gaming-cafe.kiosk.device_status';

export function readStoredDeviceStatus(): string | null {
  try {
    return localStorage.getItem(DEVICE_STATUS_KEY);
  } catch {
    return null;
  }
}

export function storeDeviceStatus(status: string | null): void {
  try {
    if (status) localStorage.setItem(DEVICE_STATUS_KEY, status);
    else localStorage.removeItem(DEVICE_STATUS_KEY);
  } catch {
    // non-fatal
  }
}
