// store/context.ts
import { createContext, type Dispatch } from 'react';
import type { RootAction, RootState } from './rootReducer';

export type StoreContextType = {
  state: RootState;
  dispatch: Dispatch<RootAction>;
};

export const StoreContext = createContext<StoreContextType | undefined>(undefined);
