import { http } from '@gaming-cafe/utils';
import type { UnitType } from './list';

interface AddUnitRequest {
  name: string;
  abbreviation: string;
  type?: UnitType;
  description?: string;
  isActive?: boolean;
}

export const addUnit = async (unit: AddUnitRequest) => {
  return http.post('/units', unit);
};
