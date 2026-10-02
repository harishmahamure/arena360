import { panelClaims } from '../lib/authSession';
// store/rootReducer.ts
import { type AuthAction, type AuthState, authInitialState, authReducer } from './auth/action';
import { loadState } from './persistance';

export interface RootState {
  auth: AuthState;
}
const claims = panelClaims();
const saved = loadState()?.auth;
export const rootInitialState = {
  auth: claims
    ? {
        ...authInitialState,
        ...(saved?.id === claims.userId ? saved : {}),
        id: claims.userId!,
        role: claims.roles?.includes('admin') ? 'admin' : 'staff',
        isActive: true,
      }
    : authInitialState,
};

export type RootAction = AuthAction;

export function rootReducer(state: RootState = rootInitialState, action: RootAction): RootState {
  return {
    auth: authReducer(action, state.auth),
  };
}
