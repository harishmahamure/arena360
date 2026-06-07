import { useGameLibrary } from '../../hooks/useGameLibrary';
import { fetchGames } from '../../lib/allowList';
import { GameCard } from './GameCard';
import { useAllowList } from './useAllowList';
import { useLauncher } from './useLauncher';

interface LibraryViewProps {
  disabled?: boolean;
  initialQuery?: string;
  onError?: (message: string) => void;
  onLaunched?: () => void;
}

/** Arena360 Game Library: search and filter allowed games on this station. */
export function LibraryView({
  disabled,
  initialQuery = '',
  onError,
  onLaunched,
}: LibraryViewProps) {
  const { items: games } = useAllowList(fetchGames);
  const { query, setQuery, genre, setGenre, sort, setSort, visible, genres, resultLabel } =
    useGameLibrary({ games, initialQuery });
  const { launchingKey, isLaunchable, launch } = useLauncher(
    Boolean(disabled),
    onError,
    onLaunched,
  );

  return (
    <section className="a360-section">
      <header className="a360-library-head">
        <div>
          <h1 className="a360-library-title">
            Vault <span>/ Library</span>
          </h1>
          <p className="a360-library-sub">
            Browse the games allowed on this station and jump straight into gameplay.
          </p>
        </div>
        <div className="a360-library-tools">
          <div className="a360-search">
            <span className="material-symbols-outlined">search</span>
            <input
              type="search"
              value={query}
              placeholder="Search games…"
              aria-label="Search games"
              data-library-search=""
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          {genres.length > 0 ? (
            <select
              className="a360-library-filter"
              aria-label="Filter by genre"
              value={genre ?? ''}
              onChange={(e) => setGenre(e.target.value || null)}
            >
              <option value="">All genres</option>
              {genres.map((g) => (
                <option key={g} value={g}>
                  {g}
                </option>
              ))}
            </select>
          ) : null}
          <select
            className="a360-library-filter"
            aria-label="Sort games"
            value={sort}
            onChange={(e) => setSort(e.target.value as 'name' | 'sortOrder')}
          >
            <option value="sortOrder">Staff order</option>
            <option value="name">Name A–Z</option>
          </select>
        </div>
      </header>

      {games.length > 0 ? (
        <p className="a360-library-count" aria-live="polite">
          {resultLabel}
        </p>
      ) : null}

      {games.length === 0 ? (
        <p className="a360-empty">No games allowed yet. Ask staff to set up this station.</p>
      ) : visible.length === 0 ? (
        <p className="a360-empty">No games match your search.</p>
      ) : (
        <div className="a360-grid">
          {visible.map((entry) => (
            <GameCard
              key={entry.id}
              entry={entry}
              launchable={isLaunchable(entry)}
              launching={launchingKey === entry.id}
              disabled={disabled}
              onLaunch={() => void launch(entry)}
            />
          ))}
        </div>
      )}
    </section>
  );
}
