/**
 * Array utility functions
 */

/**
 * Chunk array into smaller arrays
 */
export const chunk = <T>(array: T[], size: number): T[][] => {
  const chunks: T[][] = [];
  for (let i = 0; i < array.length; i += size) {
    chunks.push(array.slice(i, i + size));
  }
  return chunks;
};

/**
 * Remove duplicates from array
 */
export const unique = <T>(array: T[]): T[] => {
  return [...new Set(array)];
};

/**
 * Remove duplicates by key
 */
export const uniqueBy = <T>(array: T[], key: keyof T): T[] => {
  const seen = new Set();
  return array.filter((item) => {
    const value = item[key];
    if (seen.has(value)) {
      return false;
    }
    seen.add(value);
    return true;
  });
};

/**
 * Shuffle array
 */
export const shuffle = <T>(array: T[]): T[] => {
  const shuffled = [...array];
  for (let i = shuffled.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [shuffled[i]!, shuffled[j]!] = [shuffled[j]!, shuffled[i]!];
  }
  return shuffled;
};

/**
 * Get random item from array
 */
export const randomItem = <T>(array: T[]): T | undefined => {
  return array[Math.floor(Math.random() * array.length)];
};

/**
 * Group array by key
 */
export const groupBy = <T>(array: T[], key: keyof T): Record<string, T[]> => {
  return array.reduce(
    (groups, item) => {
      const value = String(item[key]);
      if (!groups[value]) {
        groups[value] = [];
      }
      groups[value].push(item);
      return groups;
    },
    {} as Record<string, T[]>,
  );
};

/**
 * Sort array of objects by key
 */
export const sortBy = <T>(array: T[], key: keyof T, order: 'asc' | 'desc' = 'asc'): T[] => {
  return [...array].sort((a, b) => {
    const aVal = a[key];
    const bVal = b[key];

    if (aVal < bVal) return order === 'asc' ? -1 : 1;
    if (aVal > bVal) return order === 'asc' ? 1 : -1;
    return 0;
  });
};

/**
 * Flatten nested array
 */
export const flatten = <T>(array: unknown[]): T[] => {
  return array.flat(Infinity) as T[];
};

/**
 * Flatten array one level deep
 */
export const flattenOnce = <T>(array: T[][]): T[] => {
  return array.flat() as T[];
};

/**
 * Check if array is empty
 */
export const isEmpty = <T>(array: T[]): boolean => {
  return array.length === 0;
};

/**
 * Get first item
 */
export const first = <T>(array: T[]): T | undefined => {
  return array[0];
};

/**
 * Get last item
 */
export const last = <T>(array: T[]): T | undefined => {
  return array[array.length - 1];
};

/**
 * Remove item at index
 */
export const removeAt = <T>(array: T[], index: number): T[] => {
  return [...array.slice(0, index), ...array.slice(index + 1)];
};

/**
 * Insert item at index
 */
export const insertAt = <T>(array: T[], index: number, item: T): T[] => {
  return [...array.slice(0, index), item, ...array.slice(index)];
};

/**
 * Move item from one index to another
 */
export const moveItem = <T>(array: T[], from: number, to: number): T[] => {
  const item = array[from];
  if (item === undefined) {
    return array;
  }
  const withoutItem = removeAt(array, from);
  return insertAt(withoutItem, to, item);
};

/**
 * Partition array into two arrays based on predicate
 */
export const partition = <T>(array: T[], predicate: (item: T) => boolean): [T[], T[]] => {
  const pass: T[] = [];
  const fail: T[] = [];

  array.forEach((item) => {
    if (predicate(item)) {
      pass.push(item);
    } else {
      fail.push(item);
    }
  });

  return [pass, fail];
};

/**
 * Count occurrences of items in array
 */
export const countBy = <T>(array: T[], key?: keyof T): Record<string, number> => {
  return array.reduce(
    (counts, item) => {
      const value = key ? String(item[key]) : String(item);
      counts[value] = (counts[value] || 0) + 1;
      return counts;
    },
    {} as Record<string, number>,
  );
};

/**
 * Find differences between two arrays
 */
export const difference = <T>(array1: T[], array2: T[]): T[] => {
  return array1.filter((item) => !array2.includes(item));
};

/**
 * Find intersection of two arrays
 */
export const intersection = <T>(array1: T[], array2: T[]): T[] => {
  return array1.filter((item) => array2.includes(item));
};

/**
 * Combine two arrays without duplicates
 */
export const union = <T>(array1: T[], array2: T[]): T[] => {
  return unique([...array1, ...array2]);
};

/**
 * Check if arrays are equal
 */
export const areEqual = <T>(array1: T[], array2: T[]): boolean => {
  if (array1.length !== array2.length) return false;
  return array1.every((item, index) => item === array2[index]);
};

/**
 * Paginate array
 */
export const paginate = <T>(array: T[], page: number, pageSize: number): T[] => {
  const start = (page - 1) * pageSize;
  return array.slice(start, start + pageSize);
};

/**
 * Create array of numbers in range
 */
export const range = (start: number, end: number, step = 1): number[] => {
  const result: number[] = [];
  for (let i = start; i <= end; i += step) {
    result.push(i);
  }
  return result;
};

/**
 * Sum array of numbers
 */
export const sumArray = (numbers: number[]): number => {
  return numbers.reduce((total, num) => total + num, 0);
};

/**
 * Get max value from array of objects
 */
export const maxBy = <T>(array: T[], key: keyof T): T | undefined => {
  if (array.length === 0) return undefined;
  return array.reduce((max, item) => (item[key] > max[key] ? item : max));
};

/**
 * Get min value from array of objects
 */
export const minBy = <T>(array: T[], key: keyof T): T | undefined => {
  if (array.length === 0) return undefined;
  return array.reduce((min, item) => (item[key] < min[key] ? item : min));
};
