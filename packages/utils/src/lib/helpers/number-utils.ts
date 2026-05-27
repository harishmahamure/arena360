/**
 * Number utility functions
 */

/**
 * Format number as currency
 */
export const formatCurrency = (amount: number, currency = 'INR', locale = 'en-IN'): string => {
  return new Intl.NumberFormat(locale, {
    style: 'currency',
    currency,
  }).format(amount);
};

/**
 * Format number with thousand separators
 */
export const formatNumber = (num: number, locale = 'en-IN'): string => {
  return new Intl.NumberFormat(locale).format(num);
};

/**
 * Format number as percentage
 */
export const formatPercentage = (num: number, decimals = 2, locale = 'en-IN'): string => {
  return new Intl.NumberFormat(locale, {
    style: 'percent',
    minimumFractionDigits: decimals,
    maximumFractionDigits: decimals,
  }).format(num);
};

/**
 * Round number to specified decimal places
 */
export const round = (num: number, decimals = 2): number => {
  return Math.round(num * 10 ** decimals) / 10 ** decimals;
};

/**
 * Clamp number between min and max
 */
export const clamp = (num: number, min: number, max: number): number => {
  return Math.min(Math.max(num, min), max);
};

/**
 * Check if number is in range
 */
export const inRange = (num: number, min: number, max: number): boolean => {
  return num >= min && num <= max;
};

/**
 * Generate random number between min and max
 */
export const randomInt = (min: number, max: number): number => {
  return Math.floor(Math.random() * (max - min + 1)) + min;
};

/**
 * Generate random float between min and max
 */
export const randomFloat = (min: number, max: number, decimals = 2): number => {
  return round(Math.random() * (max - min) + min, decimals);
};

/**
 * Calculate percentage
 */
export const percentage = (value: number, total: number): number => {
  if (total === 0) return 0;
  return (value / total) * 100;
};

/**
 * Calculate average
 */
export const average = (numbers: number[]): number => {
  if (numbers.length === 0) return 0;
  return numbers.reduce((sum, num) => sum + num, 0) / numbers.length;
};

/**
 * Calculate sum
 */
export const sum = (numbers: number[]): number => {
  return numbers.reduce((total, num) => total + num, 0);
};

/**
 * Find minimum value
 */
export const min = (numbers: number[]): number => {
  return Math.min(...numbers);
};

/**
 * Find maximum value
 */
export const max = (numbers: number[]): number => {
  return Math.max(...numbers);
};

/**
 * Format file size
 */
export const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 Bytes';

  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));

  return round(bytes / k ** i, 2) + ' ' + sizes[i];
};

/**
 * Convert bytes to megabytes
 */
export const bytesToMB = (bytes: number): number => {
  return round(bytes / (1024 * 1024), 2);
};

/**
 * Convert bytes to gigabytes
 */
export const bytesToGB = (bytes: number): number => {
  return round(bytes / (1024 * 1024 * 1024), 2);
};

/**
 * Check if number is even
 */
export const isEven = (num: number): boolean => {
  return num % 2 === 0;
};

/**
 * Check if number is odd
 */
export const isOdd = (num: number): boolean => {
  return num % 2 !== 0;
};

/**
 * Abbreviate large numbers (e.g., 1000 -> 1K)
 */
export const abbreviateNumber = (num: number): string => {
  if (num < 1000) return num.toString();
  if (num < 1000000) return round(num / 1000, 1) + 'K';
  if (num < 1000000000) return round(num / 1000000, 1) + 'M';
  return round(num / 1000000000, 1) + 'B';
};

/**
 * Parse string to number safely
 */
export const parseNumber = (value: string | number): number | null => {
  if (typeof value === 'number') return value;
  const parsed = parseFloat(value);
  return isNaN(parsed) ? null : parsed;
};

/**
 * Format number with ordinal suffix (1st, 2nd, 3rd, etc.)
 */
export const ordinalSuffix = (num: number): string => {
  const j = num % 10;
  const k = num % 100;

  if (j === 1 && k !== 11) return num + 'st';
  if (j === 2 && k !== 12) return num + 'nd';
  if (j === 3 && k !== 13) return num + 'rd';
  return num + 'th';
};
