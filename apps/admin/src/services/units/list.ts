import type { UnitTypeValue } from '@gaming-cafe/contracts';
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
  type: UnitTypeValue;
  description?: string;
  isActive: boolean;
}

export { UnitType } from '@gaming-cafe/contracts';
export type { UnitTypeValue };

export interface GetUnitsFilters {
  name?: string;
  type?: UnitTypeValue;
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
