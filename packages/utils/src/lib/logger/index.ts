/**
 * Logger utility for development and production environments
 * In production builds with Vite, all console statements are automatically stripped
 */

const isDevelopment = import.meta.env.DEV;

export const logger = {
  /**
   * Log general information (only in development)
   */
  log: (...args: any[]) => {
    if (isDevelopment) {
      console.log(...args);
    }
  },

  /**
   * Log informational messages (only in development)
   */
  info: (...args: any[]) => {
    if (isDevelopment) {
      console.info(...args);
    }
  },

  /**
   * Log warning messages (only in development)
   */
  warn: (...args: any[]) => {
    if (isDevelopment) {
      console.warn(...args);
    }
  },

  /**
   * Log error messages (always logged, even in production)
   * Note: In production builds, these will be stripped by Vite's esbuild config
   */
  error: (...args: any[]) => {
    console.error(...args);
  },

  /**
   * Log debug messages (only in development)
   */
  debug: (...args: any[]) => {
    if (isDevelopment) {
      console.debug(...args);
    }
  },

  /**
   * Log a table (only in development)
   */
  table: (data: any) => {
    if (isDevelopment && console.table) {
      console.table(data);
    }
  },

  /**
   * Start a timer (only in development)
   */
  time: (label: string) => {
    if (isDevelopment && console.time) {
      console.time(label);
    }
  },

  /**
   * End a timer (only in development)
   */
  timeEnd: (label: string) => {
    if (isDevelopment && console.timeEnd) {
      console.timeEnd(label);
    }
  },

  /**
   * Group console logs (only in development)
   */
  group: (label: string) => {
    if (isDevelopment && console.group) {
      console.group(label);
    }
  },

  /**
   * Group collapsed console logs (only in development)
   */
  groupCollapsed: (label: string) => {
    if (isDevelopment && console.groupCollapsed) {
      console.groupCollapsed(label);
    }
  },

  /**
   * End a group (only in development)
   */
  groupEnd: () => {
    if (isDevelopment && console.groupEnd) {
      console.groupEnd();
    }
  },
};

export default logger;
