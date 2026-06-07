import type { LaunchEntry } from './allowList';

export type GameLibrarySort = 'name' | 'sortOrder';

export interface GameLibraryFilters {
  query: string;
  genre?: string | null;
  sort: GameLibrarySort;
}

function byName(a: LaunchEntry, b: LaunchEntry): number {
  return a.name.localeCompare(b.name);
}

function bySortOrder(a: LaunchEntry, b: LaunchEntry): number {
  const order = (a.sortOrder ?? 0) - (b.sortOrder ?? 0);
  return order !== 0 ? order : byName(a, b);
}

/** Match query against game name (word tokens + substring). */
export function matchesGameQuery(name: string, query: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  const normalized = name.toLowerCase();
  if (normalized.includes(q)) return true;
  const tokens = q.split(/\s+/).filter(Boolean);
  return tokens.every((token) => normalized.includes(token));
}

/** Distinct non-empty genres from the game list, sorted. */
export function collectGameGenres(games: LaunchEntry[]): string[] {
  const genres = new Set<string>();
  for (const game of games) {
    const genre = game.genre?.trim();
    if (genre) genres.add(genre);
  }
  return [...genres].sort((a, b) => a.localeCompare(b));
}

/** Filter and sort games for the library grid. */
export function filterGameLibrary(
  games: LaunchEntry[],
  filters: GameLibraryFilters,
): LaunchEntry[] {
  const genre = filters.genre?.trim() || null;
  let result = games.filter((game) => {
    if (genre && (game.genre?.trim() ?? '') !== genre) return false;
    return matchesGameQuery(game.name, filters.query);
  });
  result = [...result].sort(filters.sort === 'name' ? byName : bySortOrder);
  return result;
}
