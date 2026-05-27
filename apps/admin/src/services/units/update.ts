import { http } from '@gaming-cafe/utils';
import type { CreateUnitFormData } from '../../containers/units/schemas/unit-schema';

export const updateUnit = async (id: string, unit: CreateUnitFormData) => {
  return http.put(`/units/${id}`, unit);
};
