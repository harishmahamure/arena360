/**
 * Local Storage and Session Storage utility functions
 */

/**
 * Storage type
 */
type StorageType = 'local' | 'session';

/**
 * Get storage instance
 */
const getStorage = (type: StorageType): Storage | null => {
  if (typeof window === 'undefined') return null;
  return type === 'local' ? localStorage : sessionStorage;
};

/**
 * Set item in storage
 */
export const setStorageItem = <T>(key: string, value: T, type: StorageType = 'local'): void => {
  try {
    const storage = getStorage(type);
    if (!storage) return;
    storage.setItem(key, JSON.stringify(value));
  } catch (_error) {}
};

/**
 * Get item from storage
 */
export const getStorageItem = <T>(key: string, type: StorageType = 'local'): T | null => {
  try {
    const storage = getStorage(type);
    if (!storage) return null;

    const item = storage.getItem(key);
    return item ? JSON.parse(item) : null;
  } catch (_error) {
    return null;
  }
};

/**
 * Remove item from storage
 */
export const removeStorageItem = (key: string, type: StorageType = 'local'): void => {
  try {
    const storage = getStorage(type);
    if (!storage) return;
    storage.removeItem(key);
  } catch (_error) {}
};

/**
 * Clear all items from storage
 */
export const clearStorage = (type: StorageType = 'local'): void => {
  try {
    const storage = getStorage(type);
    if (!storage) return;
    storage.clear();
  } catch (_error) {}
};

/**
 * Check if item exists in storage
 */
export const hasStorageItem = (key: string, type: StorageType = 'local'): boolean => {
  const storage = getStorage(type);
  if (!storage) return false;
  return storage.getItem(key) !== null;
};

/**
 * Get all keys from storage
 */
export const getStorageKeys = (type: StorageType = 'local'): string[] => {
  const storage = getStorage(type);
  if (!storage) return [];
  return Object.keys(storage);
};

/**
 * Get storage size in bytes
 */
export const getStorageSize = (type: StorageType = 'local'): number => {
  const storage = getStorage(type);
  if (!storage) return 0;

  let size = 0;
  for (const key in storage) {
    if (Object.hasOwn(storage, key)) {
      size += key.length + (storage[key]?.length || 0);
    }
  }
  return size;
};

/**
 * Set item with expiration
 */
export const setStorageItemWithExpiry = <T>(
  key: string,
  value: T,
  expiryMs: number,
  type: StorageType = 'local',
): void => {
  const item = {
    value,
    expiry: Date.now() + expiryMs,
  };
  setStorageItem(key, item, type);
};

/**
 * Get item with expiration check
 */
export const getStorageItemWithExpiry = <T>(key: string, type: StorageType = 'local'): T | null => {
  const item = getStorageItem<{ value: T; expiry: number }>(key, type);

  if (!item) return null;

  if (Date.now() > item.expiry) {
    removeStorageItem(key, type);
    return null;
  }

  return item.value;
};

/**
 * Local Storage helpers
 */
export const local = {
  set: <T>(key: string, value: T) => setStorageItem(key, value, 'local'),
  get: <T>(key: string) => getStorageItem<T>(key, 'local'),
  remove: (key: string) => removeStorageItem(key, 'local'),
  clear: () => clearStorage('local'),
  has: (key: string) => hasStorageItem(key, 'local'),
  keys: () => getStorageKeys('local'),
  size: () => getStorageSize('local'),
  setWithExpiry: <T>(key: string, value: T, expiryMs: number) =>
    setStorageItemWithExpiry(key, value, expiryMs, 'local'),
  getWithExpiry: <T>(key: string) => getStorageItemWithExpiry<T>(key, 'local'),
};

/**
 * Session Storage helpers
 */
export const session = {
  set: <T>(key: string, value: T) => setStorageItem(key, value, 'session'),
  get: <T>(key: string) => getStorageItem<T>(key, 'session'),
  remove: (key: string) => removeStorageItem(key, 'session'),
  clear: () => clearStorage('session'),
  has: (key: string) => hasStorageItem(key, 'session'),
  keys: () => getStorageKeys('session'),
  size: () => getStorageSize('session'),
  setWithExpiry: <T>(key: string, value: T, expiryMs: number) =>
    setStorageItemWithExpiry(key, value, expiryMs, 'session'),
  getWithExpiry: <T>(key: string) => getStorageItemWithExpiry<T>(key, 'session'),
};
