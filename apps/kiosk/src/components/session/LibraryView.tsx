import { useMemo, useState } from 'react';
import { fetchActiveGames } from '../../lib/games';
import { GameCard } from './GameCard';
import { useCatalog } from './useCatalog';
import { useLauncher } from './useLauncher';

interface LibraryViewProps {
  disabled?: boolean;
  onError?: (message: string) => void;
}

/** Arena360 Game Library: search across the games in the catalog. */
export function LibraryView({ disabled, onError }: LibraryViewProps) {
  const { items: games } = useCatalog(fetchActiveGames);
  const [query, setQuery] = useState('');
  const { launchingKey, isLaunchable, launch } = useLauncher(Boolean(disabled), onError);

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
            Browse the games installed on this station and jump straight into gameplay.
          </p>
        </div>
        <div className="a360-library-tools">
          <div className="a360-search">
            <span className="material-symbols-outlined">search</span>
            <input
              type="search"
              value={query}
              placeholder="Search installed games…"
              aria-label="Search games"
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
        </div>
      </header>

      {games.length === 0 ? (
        <p className="a360-empty">No games installed. Ask staff to set up this station.</p>
      ) : visible.length === 0 ? (
        <p className="a360-empty">No games match your search.</p>
      ) : (
        <div className="a360-grid">
          {visible.map((game) => (
            <GameCard
              key={game.id}
              game={game}
              launchable={isLaunchable(game)}
              launching={launchingKey === game.id}
              disabled={disabled}
              onLaunch={() => void launch(game)}
            />
          ))}
        </div>
      )}
    </section>
  );
}
