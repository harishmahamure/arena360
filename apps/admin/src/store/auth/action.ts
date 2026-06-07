export interface AuthState {
  id: string;
  email: string;
  username: string;
  firstName: string;
  lastName: string;
  role: string;
  isActive: boolean;
}

export type AuthAction =
  | { type: 'GetAuthDetails' }
  | { type: 'SetAuthDetail'; payload: AuthState }
  | { type: 'Reset' };

export const authInitialState: AuthState = {
  id: '',
  email: '',
  firstName: '',
  lastName: '',
  isActive: false,
  username: '',
  role: '',
};

export const authReducer = (action: AuthAction, state: AuthState): AuthState => {
  const { type } = action;
  switch (type) {
    case 'GetAuthDetails': {
      return state;
    }
    case 'SetAuthDetail': {
      return {
        ...state,
        ...action.payload,
        role: action.payload.role?.toLowerCase() ?? '',
      };
    }
    case 'Reset': {
      return {
        ...authInitialState,
      };
    }
    default: {
      return state;
    }
  }
};
