const TIME_OF_DAY_RE = /^([0-1]?[0-9]|2[0-3]):([0-5][0-9])(?::([0-5][0-9]))?$/;

/** Normalize HH:MM to HH:MM:SS for API payloads. */
export function normalizeTimeOfDay(value?: string | null): string {
  if (!value) return '';
  return value.length === 5 ? `${value}:00` : value;
}

/** HH:MM or HH:MM:SS → HH:MM for native `<input type="time">`. */
export function toTimeInputValue(value?: string | null): string {
  if (!value) return '';
  const match = value.match(TIME_OF_DAY_RE);
  if (!match) return '';
  const hours = match[1]?.padStart(2, '0') ?? '00';
  const minutes = match[2] ?? '00';
  return `${hours}:${minutes}`;
}

/** Parse HH:MM or HH:MM:SS into a Date anchored to today (for time pickers). */
export function parseTimeOfDay(value?: string | null): Date | null {
  if (!value) return null;
  const match = value.match(TIME_OF_DAY_RE);
  if (!match) return null;

  const hours = Number(match[1] ?? 0);
  const minutes = Number(match[2] ?? 0);
  const seconds = match[3] ? Number(match[3]) : 0;
  const date = new Date();
  date.setHours(hours, minutes, seconds, 0);
  return date;
}

/** Format a Date as HH:MM:SS for API storage. */
export function formatTimeOfDay(date: Date | null): string {
  if (!date || Number.isNaN(date.getTime())) return '';
  const hours = String(date.getHours()).padStart(2, '0');
  const minutes = String(date.getMinutes()).padStart(2, '0');
  const seconds = String(date.getSeconds()).padStart(2, '0');
  return `${hours}:${minutes}:${seconds}`;
}
