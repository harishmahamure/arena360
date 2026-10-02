import { local } from '@gaming-cafe/utils';
import type { RootState } from './rootReducer';

export const PERSIST_KEY = 'state';

export function loadState(): Partial<RootState> | undefined {
  try {
    const raw = local.get(PERSIST_KEY) as string;
    if (!raw) return undefined;
    return JSON.parse(raw) as Partial<RootState>;
  } catch {
    return undefined;
  }
}
