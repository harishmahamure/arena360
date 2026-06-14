import type { UnitTypeValue } from '@gaming-cafe/contracts';
import { http } from '@gaming-cafe/utils';

interface AddUnitRequest {
  name: string;
  abbreviation: string;
  type?: UnitTypeValue;
  description?: string;
  isActive?: boolean;
}

export const addUnit = async (unit: AddUnitRequest) => {
  return http.post('/units', unit);
};
