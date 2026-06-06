import { useMemo, useState } from 'react';
import { fetchGames } from '../../lib/allowList';
import { GameCard } from './GameCard';
import { useAllowList } from './useAllowList';
import { useLauncher } from './useLauncher';

interface LibraryViewProps {
  disabled?: boolean;
  onError?: (message: string) => void;
  onLaunched?: () => void;
}

/** Arena360 Game Library: search across allowed games on this station. */
export function LibraryView({ disabled, onError, onLaunched }: LibraryViewProps) {
  const { items: games } = useAllowList(fetchGames);
  const [query, setQuery] = useState('');
  const { launchingKey, isLaunchable, launch } = useLauncher(
    Boolean(disabled),
    onError,
    onLaunched,
  );

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return games.filter((g) => !q || g.name.toLowerCase().includes(q));
  }, [games, query]);

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
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
        </div>
      </header>

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
