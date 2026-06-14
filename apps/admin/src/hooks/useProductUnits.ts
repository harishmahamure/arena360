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
const canonicalTypeOrder = CANONICAL_UNITS.map(({ type }) => type);

function sortUnitsByCanonicalOrder<T extends { type: string }>(units: T[]): T[] {
  return [...units].sort((a, b) => {
    const ai = canonicalTypeOrder.indexOf(a.type as UnitTypeValue);
    const bi = canonicalTypeOrder.indexOf(b.type as UnitTypeValue);
    return (ai === -1 ? 999 : ai) - (bi === -1 ? 999 : bi);
  });
}

export function useProductUnits() {
  const { data, isSuccess, isLoading } = useQuery({
    queryKey: ['units-for-product'],
    queryFn: () => getUnits({ limit: 100, isActive: true }),
    staleTime: 1000 * 60 * 5,
  });

  const activeUnits = useMemo(
    () => sortUnitsByCanonicalOrder((data?.data ?? []).filter((unit) => unit.isActive)),
    [data],
  );

  const unitsByType = useMemo(() => {
    const map = new Map<UnitTypeValue, string>();
    for (const unit of activeUnits) {
      map.set(unit.type as UnitTypeValue, unit.id);
    }
    return map;
  }, [activeUnits]);

  const unitSelectOptions: FormSelectOption[] = useMemo(
    () =>
      activeUnits.map((unit) => ({
        value: unit.id,
        label: labelByType.get(unit.type as UnitTypeValue) ?? unit.name,
      })),
    [activeUnits],
  );

  const defaultUnitIds = useMemo(
    () => ({
      sale: unitsByType.get(UnitType.PIECE),
      purchase: unitsByType.get(UnitType.BOX),
    }),
    [unitsByType],
  );

  return {
    unitSelectOptions,
    defaultUnitIds,
    unitsReady: isSuccess && unitSelectOptions.length > 0,
    unitsMissing: isSuccess && unitSelectOptions.length === 0,
    unitsLoading: isLoading,
  };
}
