import { http } from '@gaming-cafe/utils';

interface GetUnitsResponse {
  data: UnitResponse[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface UnitResponse {
  id: string;
  createdAt: string;
  updatedAt: string;
  name: string;
  abbreviation: string;
  type: UnitType;
  description?: string;
  isActive: boolean;
}

export enum UnitType {
  BOX = 'box',
  KILOGRAM = 'kilogram',
  GRAM = 'gram',
  LITER = 'liter',
  MILLILITER = 'milliliter',
  PIECE = 'piece',
  PACK = 'pack',
  DOZEN = 'dozen',
  OTHER = 'other',
}

export interface GetUnitsFilters {
  name?: string;
  type?: UnitType;
  isActive?: boolean;
  page?: number;
  limit?: number;
  sort?: 'asc' | 'desc';
}

export const getUnits = async (filters: GetUnitsFilters) => {
  return http.get<GetUnitsResponse>('/units', {
    params: {
      ...filters,
    },
  });
};
