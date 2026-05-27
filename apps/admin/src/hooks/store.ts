// store/hooks.ts
import { useContext } from 'react';
import { StoreContext } from '../store';
import type { RootState } from '../store/rootReducer';

export function useSelector<T>(selector: (state: RootState) => T): T {
  const context = useContext(StoreContext);
  if (!context) throw new Error('useSelector must be inside Provider');
  return selector(context.state);
}

export function useDispatch() {
  const context = useContext(StoreContext);
  if (!context) throw new Error('useDispatch must be inside Provider');
  return context.dispatch;
}
