import {
  CANONICAL_UNITS,
  UnitType,
  type UnitTypeValue,
  unitTypeOptions,
} from '@gaming-cafe/contracts';
import type { FormSelectOption } from '@gaming-cafe/ui';
import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getUnits } from '../services/units/list';

const labelByType = new Map(unitTypeOptions.map((o) => [o.value, o.label]));

export function useProductUnits() {
  const { data, isSuccess } = useQuery({
    queryKey: ['units-for-product'],
    queryFn: () => getUnits({ limit: 100, isActive: true }),
  });

  const unitsByType = useMemo(() => {
    const map = new Map<UnitTypeValue, string>();
    for (const unit of data?.data ?? []) {
      map.set(unit.type as UnitTypeValue, unit.id);
    }
    return map;
  }, [data]);

  const unitSelectOptions: FormSelectOption[] = useMemo(
    () =>
      CANONICAL_UNITS.map(({ type }) => {
        const id = unitsByType.get(type);
        return {
          value: id ?? type,
          label: labelByType.get(type) ?? type,
          disabled: !id,
        };
      }),
    [unitsByType],
  );

  const defaultUnitIds = useMemo(
    () => ({
      sale: unitsByType.get(UnitType.PIECE),
      purchase: unitsByType.get(UnitType.BOX),
    }),
    [unitsByType],
  );

  return { unitSelectOptions, defaultUnitIds, unitsReady: isSuccess };
}
