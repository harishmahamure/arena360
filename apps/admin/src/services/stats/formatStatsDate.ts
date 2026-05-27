import { toISTString } from '../../utils/date';

/**
 * Format a date for backend stats query params as ISO 8601 with IST offset.
 */
export const formatStatsDate = (date: Date): string => toISTString(date);
