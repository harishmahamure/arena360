const IST_OFFSET_MINUTES = 330; // UTC+5:30
const IST_TZ = 'Asia/Kolkata';
const LOCALE = 'en-IN';

/**
 * Get current time as a Date object. Wrapper for testability.
 */
export function now(): Date {
  return new Date();
}

/**
 * Start of today in IST, returned as a Date object.
 */
export function startOfTodayIST(): Date {
  const d = now();
  const istDate = new Date(d.getTime() + (IST_OFFSET_MINUTES + d.getTimezoneOffset()) * 60000);
  istDate.setHours(0, 0, 0, 0);
  return new Date(istDate.getTime() - (IST_OFFSET_MINUTES + d.getTimezoneOffset()) * 60000);
}

/**
 * End of today in IST (23:59:59.999), returned as a Date object.
 */
export function endOfTodayIST(): Date {
  const d = now();
  const istDate = new Date(d.getTime() + (IST_OFFSET_MINUTES + d.getTimezoneOffset()) * 60000);
  istDate.setHours(23, 59, 59, 999);
  return new Date(istDate.getTime() - (IST_OFFSET_MINUTES + d.getTimezoneOffset()) * 60000);
}

/**
 * Format a Date as an ISO 8601 string with IST offset for API queries.
 * Example: "2026-05-27T00:00:00+05:30"
 */
export function toISTString(date: Date): string {
  const istMs = date.getTime() + IST_OFFSET_MINUTES * 60000;
  const ist = new Date(istMs);

  const year = ist.getUTCFullYear();
  const month = String(ist.getUTCMonth() + 1).padStart(2, '0');
  const day = String(ist.getUTCDate()).padStart(2, '0');
  const hours = String(ist.getUTCHours()).padStart(2, '0');
  const minutes = String(ist.getUTCMinutes()).padStart(2, '0');
  const seconds = String(ist.getUTCSeconds()).padStart(2, '0');

  return `${year}-${month}-${day}T${hours}:${minutes}:${seconds}+05:30`;
}

/**
 * Format a date/ISO string for display: "27 May 2026"
 */
export function formatDisplayDate(value: string | Date): string {
  const date = typeof value === 'string' ? new Date(value) : value;
  return date.toLocaleDateString(LOCALE, {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    timeZone: IST_TZ,
  });
}

/**
 * Format a date/ISO string for display with time: "27 May 2026, 7:30 PM"
 */
export function formatDisplayDateTime(value: string | Date): string {
  const date = typeof value === 'string' ? new Date(value) : value;
  return date.toLocaleString(LOCALE, {
    day: 'numeric',
    month: 'short',
    year: 'numeric',
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
    timeZone: IST_TZ,
  });
}

/**
 * Format a date/ISO string for display as time only: "7:30 PM"
 */
export function formatDisplayTime(value: string | Date): string {
  const date = typeof value === 'string' ? new Date(value) : value;
  return date.toLocaleTimeString(LOCALE, {
    hour: 'numeric',
    minute: '2-digit',
    hour12: true,
    timeZone: IST_TZ,
  });
}

/**
 * Relative time display: "2 hours ago", "3 days ago", etc.
 * Falls back to formatDisplayDateTime for dates older than 7 days.
 */
export function formatRelativeTime(value: string | Date): string {
  const date = typeof value === 'string' ? new Date(value) : value;
  const diffMs = now().getTime() - date.getTime();
  const diffMinutes = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMs / 3600000);
  const diffDays = Math.floor(diffMs / 86400000);

  if (diffMinutes < 1) return 'Just now';
  if (diffMinutes < 60) return `${diffMinutes} min ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;
  return formatDisplayDateTime(date);
}

/**
 * Format elapsed duration in human-readable form: "2h 15m" or "45m"
 */
export function formatDuration(minutes: number): string {
  if (minutes < 60) return `${Math.round(minutes)}m`;
  const h = Math.floor(minutes / 60);
  const m = Math.round(minutes % 60);
  return m > 0 ? `${h}h ${m}m` : `${h}h`;
}
