// store/rootReducer.ts
import { type AuthAction, type AuthState, authInitialState, authReducer } from './auth/action';
import { loadState } from './persistance';

const persistedState = typeof window !== 'undefined' ? loadState() : undefined;

export interface RootState {
  auth: AuthState;
}

export const rootInitialState = {
  auth: {
    ...authInitialState,
    ...(persistedState?.auth || {}),
  },
};

export type RootAction = AuthAction;

export function rootReducer(state: RootState = rootInitialState, action: RootAction): RootState {
  return {
    auth: authReducer(action, state.auth),
  };
}
