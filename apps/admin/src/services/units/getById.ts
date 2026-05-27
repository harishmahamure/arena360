import { http } from '@gaming-cafe/utils';
import type { UnitResponse } from './list';

export const getUnitById = async (id: string) => {
  return http.get<UnitResponse>(`/units/${id}`);
};
