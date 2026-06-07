import { useEffect, useMemo, useState } from 'react';
import type { LaunchEntry } from '../lib/allowList';
import {
  collectGameGenres,
  filterGameLibrary,
  type GameLibraryFilters,
  type GameLibrarySort,
} from '../lib/gameLibrary';

const DEBOUNCE_MS = 150;

export interface UseGameLibraryOptions {
  games: LaunchEntry[];
  initialQuery?: string;
}

export function useGameLibrary({ games, initialQuery = '' }: UseGameLibraryOptions) {
  const [query, setQuery] = useState(initialQuery);
  const [debouncedQuery, setDebouncedQuery] = useState(initialQuery);
  const [genre, setGenre] = useState<string | null>(null);
  const [sort, setSort] = useState<GameLibrarySort>('sortOrder');

  useEffect(() => {
    setQuery(initialQuery);
    setDebouncedQuery(initialQuery);
  }, [initialQuery]);

  useEffect(() => {
    const id = setTimeout(() => setDebouncedQuery(query), DEBOUNCE_MS);
    return () => clearTimeout(id);
  }, [query]);

  const filters: GameLibraryFilters = useMemo(
    () => ({ query: debouncedQuery, genre, sort }),
    [debouncedQuery, genre, sort],
  );

  const visible = useMemo(() => filterGameLibrary(games, filters), [games, filters]);
  const genres = useMemo(() => collectGameGenres(games), [games]);

  const resultLabel =
    debouncedQuery.trim() || genre
      ? `${visible.length} match${visible.length === 1 ? '' : 'es'}`
      : `${games.length} game${games.length === 1 ? '' : 's'}`;

  return {
    query,
    setQuery,
    genre,
    setGenre,
    sort,
    setSort,
    visible,
    genres,
    totalCount: games.length,
    resultLabel,
  };
}
