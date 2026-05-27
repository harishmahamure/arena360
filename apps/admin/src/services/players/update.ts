import type { UserRole } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

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
