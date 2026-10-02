import type { UserRole } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

export interface PlayerResponse {
  id: string;
  username: string;
  phoneNumber?: string;
  firstName?: string;
  lastName?: string;
  role: UserRole;
  isActive: boolean;
  totpEnabled?: boolean;
  creditLimit?: number;
  createdAt: string;
  updatedAt: string;
}

interface GetPlayersResponse {
  data: PlayerResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface GetPlayersFilters {
  username?: string;
  phoneNumber?: string;
  isActive?: 0 | 1;
  role?: UserRole;
  page?: number;
  limit?: number;
  sortBy?: string;
  sortOrder?: 'ASC' | 'DESC';
}

export const getPlayers = async (filters?: GetPlayersFilters) => {
  return http.get<GetPlayersResponse>('/users', {
    params: filters,
  });
};
