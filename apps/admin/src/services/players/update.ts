import { http } from '@gaming-cafe/utils';
import type { UserRole } from './list';

export interface UpdatePlayerRequest {
  email?: string;
  username?: string;
  firstName?: string;
  lastName?: string;
  role?: UserRole;
  isActive?: boolean;
}

export const updatePlayer = async (id: string, player: UpdatePlayerRequest) => {
  return http.put(`/users/${id}`, player);
};
