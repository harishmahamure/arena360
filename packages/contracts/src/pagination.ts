/**
 * Shared pagination types used across all gaming-cafe workspaces.
 */

export type SortOrder = 'ASC' | 'DESC';

export interface IPaginationParams {
  page: number;
  limit: number;
  sortBy?: string;
  sortOrder?: SortOrder;
}

export interface IPaginationResult<T> {
  data: T[];
  total: number;
  page: number;
  limit: number;
  totalPages: number;
}

export interface IQueryOptions {
  skip?: number;
  take?: number;
  order?: Record<string, SortOrder>;
}

/**
 * Helper to calculate pagination metadata
 */
export function calculatePagination(
  total: number,
  page: number,
  limit: number,
): Pick<IPaginationResult<unknown>, 'total' | 'page' | 'limit' | 'totalPages'> {
  return {
    total,
    page,
    limit,
    totalPages: Math.ceil(total / limit),
  };
}

/**
 * Convert pagination params to query options (skip/take)
 */
export function toQueryOptions(params: IPaginationParams): IQueryOptions {
  const { page, limit, sortBy, sortOrder } = params;
  return {
    skip: (page - 1) * limit,
    take: limit,
    ...(sortBy ? { order: { [sortBy]: sortOrder ?? 'ASC' } } : {}),
  };
}
