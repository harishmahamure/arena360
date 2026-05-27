// store/provider.tsx
import { type ReactNode, useReducer } from 'react';
import { StoreContext } from './index';
import { rootInitialState, rootReducer } from './rootReducer';

export function StoreProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(rootReducer, rootInitialState);

  return <StoreContext.Provider value={{ state, dispatch }}>{children}</StoreContext.Provider>;
}
