import { http } from '@gaming-cafe/utils';

export interface AddPlayerRequest {
  email?: string;
  username: string;
  password: string;
  firstName?: string;
  lastName?: string;
  role?: 'player' | 'staff';
}

export const addPlayer = async (player: AddPlayerRequest) => {
  return http.post('/auth/register', player);
};
