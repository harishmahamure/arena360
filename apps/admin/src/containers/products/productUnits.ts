import { UnitType } from '@gaming-cafe/contracts';

/** Number of stock units in one purchase unit, for compatible metric units. */
export function metricUnitConversion(
  purchaseType?: string,
  stockType?: string,
): number | undefined {
  const metricUnits: Record<string, { dimension: string; scale: number }> = {
    [UnitType.KILOGRAM]: { dimension: 'mass', scale: 1000 },
    [UnitType.GRAM]: { dimension: 'mass', scale: 1 },
    [UnitType.LITER]: { dimension: 'volume', scale: 1000 },
    [UnitType.MILLILITER]: { dimension: 'volume', scale: 1 },
  };
  const purchase = purchaseType ? metricUnits[purchaseType] : undefined;
  const stock = stockType ? metricUnits[stockType] : undefined;
  return purchase && stock && purchase.dimension === stock.dimension
    ? purchase.scale / stock.scale
    : undefined;
}
