/**
 * String utility functions
 */

/**
 * Capitalize first letter of a string
 */
export const capitalize = (str: string): string => {
  if (!str) return '';
  return str.charAt(0).toUpperCase() + str.slice(1).toLowerCase();
};

/**
 * Capitalize first letter of each word
 */
export const capitalizeWords = (str: string): string => {
  if (!str) return '';
  return str
    .split(' ')
    .map((word) => capitalize(word))
    .join(' ');
};

/**
 * Convert string to camelCase
 */
export const toCamelCase = (str: string): string => {
  if (!str) return '';
  return str.toLowerCase().replace(/[^a-zA-Z0-9]+(.)/g, (_, chr) => chr.toUpperCase());
};

/**
 * Convert string to kebab-case
 */
export const toKebabCase = (str: string): string => {
  if (!str) return '';
  return str
    .replace(/([a-z])([A-Z])/g, '$1-$2')
    .replace(/[\s_]+/g, '-')
    .toLowerCase();
};

/**
 * Convert string to snake_case
 */
export const toSnakeCase = (str: string): string => {
  if (!str) return '';
  return str
    .replace(/([a-z])([A-Z])/g, '$1_$2')
    .replace(/[\s-]+/g, '_')
    .toLowerCase();
};

/**
 * Truncate string with ellipsis
 */
export const truncate = (str: string, length: number, suffix = '...'): string => {
  if (!str || str.length <= length) return str;
  return str.substring(0, length) + suffix;
};

/**
 * Remove extra whitespace
 */
export const removeExtraSpaces = (str: string): string => {
  if (!str) return '';
  return str.replace(/\s+/g, ' ').trim();
};

/**
 * Generate slug from string
 */
export const slugify = (str: string): string => {
  if (!str) return '';
  return str
    .toLowerCase()
    .trim()
    .replace(/[^\w\s-]/g, '')
    .replace(/[\s_-]+/g, '-')
    .replace(/^-+|-+$/g, '');
};

/**
 * Check if string is empty or whitespace
 */
export const isEmptyOrWhitespace = (str: string): boolean => {
  return !str || str.trim().length === 0;
};

/**
 * Count words in string
 */
export const countWords = (str: string): number => {
  if (!str) return 0;
  return str.trim().split(/\s+/).length;
};

/**
 * Generate random string
 */
export const randomString = (length: number): string => {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
  let result = '';
  for (let i = 0; i < length; i++) {
    result += chars.charAt(Math.floor(Math.random() * chars.length));
  }
  return result;
};

/**
 * Mask string (e.g., email or phone)
 */
export const maskString = (
  str: string,
  visibleStart = 3,
  visibleEnd = 3,
  maskChar = '*',
): string => {
  if (!str || str.length <= visibleStart + visibleEnd) return str;
  const start = str.substring(0, visibleStart);
  const end = str.substring(str.length - visibleEnd);
  const masked = maskChar.repeat(str.length - visibleStart - visibleEnd);
  return start + masked + end;
};

/**
 * Extract initials from name
 */
export const getInitials = (name: string, maxLength = 2): string => {
  if (!name) return '';
  const parts = name.trim().split(/\s+/);
  const initials = parts.map((part) => part.charAt(0).toUpperCase());
  return initials.slice(0, maxLength).join('');
};

/**
 * Check if string contains only numbers
 */
export const isNumeric = (str: string): boolean => {
  return /^\d+$/.test(str);
};

/**
 * Check if string is valid email
 */
export const isValidEmail = (email: string): boolean => {
  const emailRegex = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;
  return emailRegex.test(email);
};

/**
 * Check if string is valid URL
 */
export const isValidUrl = (url: string): boolean => {
  try {
    new URL(url);
    return true;
  } catch {
    return false;
  }
};

/**
 * Trim string; empty string if only whitespace.
 */
export const trimValue = (value: string | null | undefined): string => {
  if (value == null) return '';
  return value.trim();
};

/**
 * Trim string; undefined if empty after trim.
 */
export const trimOptional = (value: string | null | undefined): string | undefined => {
  const trimmed = trimValue(value);
  return trimmed === '' ? undefined : trimmed;
};

/**
 * Remove non-digit characters from a phone string.
 */
export const digitsOnly = (value: string | null | undefined): string => {
  if (!value) return '';
  return value.replace(/\D/g, '');
};

/**
 * Replace interior whitespace runs with underscore (live input sanitization).
 * Leading/trailing spaces are left for normalizeUsername to trim on submit.
 */
export const sanitizeUsernameInput = (value: string | null | undefined): string => {
  if (!value) return '';
  return value.replace(/(?<=\S)\s+(?=\S)/g, '_').replace(/\s{2,}/g, '_');
};

/**
 * Normalize username: trim and replace whitespace runs with underscore.
 */
export const normalizeUsername = (value: string | null | undefined): string => {
  return sanitizeUsernameInput(trimValue(value));
};

/**
 * Format phone number
 */
export const formatPhoneNumber = (phone: string): string => {
  const cleaned = phone.replace(/\D/g, '');
  const match = cleaned.match(/^(\d{3})(\d{3})(\d{4})$/);
  if (match) {
    return `(${match[1]}) ${match[2]}-${match[3]}`;
  }
  return phone;
};

/**
 * Pluralize word
 */
export const pluralize = (word: string, count: number, plural?: string): string => {
  if (count === 1) return word;
  return plural || `${word}s`;
};
