import { http } from '@gaming-cafe/utils';

// Enums for player
export enum UserRole {
  ADMIN = 'admin',
  PLAYER = 'player',
}

export interface PlayerResponse {
  id: string;
  email: string;
  username: string;
  firstName?: string;
  lastName?: string;
  role: UserRole;
  isActive: boolean;
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
  email?: string;
  username?: string;
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
