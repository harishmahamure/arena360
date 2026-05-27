/**
 * Object utility functions
 */

/**
 * Deep clone an object
 */
export const deepClone = <T>(obj: T): T => {
  return JSON.parse(JSON.stringify(obj));
};

/**
 * Deep merge two objects
 */
export const deepMerge = <T extends Record<string, unknown>>(target: T, source: Partial<T>): T => {
  const output: Record<string, unknown> = { ...target };

  Object.keys(source).forEach((key) => {
    const sourceValue = source[key];
    const targetValue = output[key];

    if (
      sourceValue &&
      typeof sourceValue === 'object' &&
      !Array.isArray(sourceValue) &&
      targetValue &&
      typeof targetValue === 'object' &&
      !Array.isArray(targetValue)
    ) {
      output[key] = deepMerge(
        targetValue as Record<string, unknown>,
        sourceValue as Record<string, unknown>,
      );
    } else {
      output[key] = sourceValue;
    }
  });

  return output as T;
};

/**
 * Pick specific keys from object
 */
export const pick = <T extends Record<string, unknown>, K extends keyof T>(
  obj: T,
  keys: K[],
): Pick<T, K> => {
  const result = {} as Pick<T, K>;
  keys.forEach((key) => {
    if (key in obj) {
      result[key] = obj[key];
    }
  });
  return result;
};

/**
 * Omit specific keys from object
 */
export const omit = <T extends Record<string, unknown>, K extends keyof T>(
  obj: T,
  keys: K[],
): Omit<T, K> => {
  const result = { ...obj };
  keys.forEach((key) => {
    delete result[key];
  });
  return result as Omit<T, K>;
};

/**
 * Check if object is empty
 */
export const isEmptyObject = (obj: Record<string, unknown>): boolean => {
  return Object.keys(obj).length === 0;
};

/**
 * Get nested value from object using dot notation
 */
export const getNestedValue = <T>(
  obj: Record<string, unknown>,
  path: string,
  defaultValue?: T,
): T | undefined => {
  const keys = path.split('.');
  let value: unknown = obj;

  for (const key of keys) {
    if (value && typeof value === 'object' && key in value) {
      value = (value as Record<string, unknown>)[key];
    } else {
      return defaultValue;
    }
  }

  return value as T;
};

/**
 * Set nested value in object using dot notation
 */
export const setNestedValue = <T extends Record<string, unknown>>(
  obj: T,
  path: string,
  value: unknown,
): T => {
  const keys = path.split('.');
  const result = deepClone(obj);
  let current: Record<string, unknown> = result;

  for (let i = 0; i < keys.length - 1; i++) {
    const key = keys[i];
    if (!key) {
      continue;
    }
    if (!(key in current) || typeof current[key] !== 'object') {
      current[key] = {};
    }
    current = current[key] as Record<string, unknown>;
  }

  const lastKey = keys[keys.length - 1];
  if (lastKey) {
    current[lastKey] = value;
  }
  return result;
};

/**
 * Remove null and undefined values from object
 */
export const compact = <T extends Record<string, unknown>>(obj: T): Partial<T> => {
  const result: Partial<T> = {};

  Object.entries(obj).forEach(([key, value]) => {
    if (value !== null && value !== undefined) {
      result[key as keyof T] = value as T[keyof T];
    }
  });

  return result;
};

/**
 * Invert object keys and values
 */
export const invert = <T extends Record<string, string>>(obj: T): Record<string, string> => {
  const result: Record<string, string> = {};

  Object.entries(obj).forEach(([key, value]) => {
    result[value] = key;
  });

  return result;
};

/**
 * Map object values
 */
export const mapValues = <T extends Record<string, unknown>, R>(
  obj: T,
  fn: (value: T[keyof T], key: keyof T) => R,
): Record<keyof T, R> => {
  const result = {} as Record<keyof T, R>;

  Object.entries(obj).forEach(([key, value]) => {
    result[key as keyof T] = fn(value as T[keyof T], key as keyof T);
  });

  return result;
};

/**
 * Map object keys
 */
export const mapKeys = <T extends Record<string, unknown>>(
  obj: T,
  fn: (key: keyof T) => string,
): Record<string, T[keyof T]> => {
  const result: Record<string, T[keyof T]> = {};

  Object.entries(obj).forEach(([key, value]) => {
    result[fn(key as keyof T)] = value as T[keyof T];
  });

  return result;
};

/**
 * Check if two objects are equal (shallow comparison)
 */
export const isEqual = (obj1: Record<string, unknown>, obj2: Record<string, unknown>): boolean => {
  const keys1 = Object.keys(obj1);
  const keys2 = Object.keys(obj2);

  if (keys1.length !== keys2.length) return false;

  return keys1.every((key) => obj1[key] === obj2[key]);
};

/**
 * Check if two objects are deeply equal
 */
export const isDeepEqual = (obj1: unknown, obj2: unknown): boolean => {
  if (obj1 === obj2) return true;

  if (typeof obj1 !== 'object' || typeof obj2 !== 'object' || obj1 === null || obj2 === null) {
    return false;
  }

  const keys1 = Object.keys(obj1);
  const keys2 = Object.keys(obj2);

  if (keys1.length !== keys2.length) return false;

  return keys1.every((key) =>
    isDeepEqual((obj1 as Record<string, unknown>)[key], (obj2 as Record<string, unknown>)[key]),
  );
};

/**
 * Flatten nested object
 */
export const flattenObject = (
  obj: Record<string, unknown>,
  prefix = '',
): Record<string, unknown> => {
  const result: Record<string, unknown> = {};

  Object.entries(obj).forEach(([key, value]) => {
    const newKey = prefix ? `${prefix}.${key}` : key;

    if (value && typeof value === 'object' && !Array.isArray(value)) {
      Object.assign(result, flattenObject(value as Record<string, unknown>, newKey));
    } else {
      result[newKey] = value;
    }
  });

  return result;
};

/**
 * Convert object to query string
 */
export const toQueryString = (obj: Record<string, unknown>): string => {
  const params = new URLSearchParams();

  Object.entries(obj).forEach(([key, value]) => {
    if (value !== null && value !== undefined) {
      params.append(key, String(value));
    }
  });

  return params.toString();
};

/**
 * Parse query string to object
 */
export const fromQueryString = (queryString: string): Record<string, string> => {
  const params = new URLSearchParams(queryString);
  const result: Record<string, string> = {};

  params.forEach((value, key) => {
    result[key] = value;
  });

  return result;
};
