import { getLocationStock } from '../inventory';
import { getProducts } from './list';
import { getCurrentProductPrices } from './recipe';

async function allPages<T>(
  fetchPage: (page: number) => Promise<{ data: T[]; totalPages: number }>,
): Promise<T[]> {
  const first = await fetchPage(1);
  const rows = [...first.data];
  for (let page = 2; page <= first.totalPages; page++) {
    rows.push(...(await fetchPage(page)).data);
  }
  return rows;
}

/** Resolve the whole location snapshot before displaying sellable quantities. */
export async function getPosCatalogData(locationId: string, venueLocationId?: string) {
  const [products, stock, currentPrices] = await Promise.all([
    allPages((page) =>
      getProducts({ forSale: true, limit: 100, page, sortBy: 'name', sortOrder: 'ASC' }),
    ),
    allPages((page) => getLocationStock({ locationId, limit: 100, page })),
    getCurrentProductPrices(locationId, venueLocationId),
  ]);
  return { products, stock, currentPrices };
}
