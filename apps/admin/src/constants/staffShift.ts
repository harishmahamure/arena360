export const STAFF_SHIFT_START_KEY = 'staffShiftStart';

export function getStaffShiftStart(): string | undefined {
  return localStorage.getItem(STAFF_SHIFT_START_KEY) ?? undefined;
}

export function setStaffShiftStart(isoTimestamp: string): void {
  localStorage.setItem(STAFF_SHIFT_START_KEY, isoTimestamp);
}
