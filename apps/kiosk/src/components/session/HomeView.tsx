import { fetchActiveGames } from '../../lib/games';
import { GameCard } from './GameCard';
import type { SessionView } from './SessionNav';
import { useCatalog } from './useCatalog';
import { useLauncher } from './useLauncher';

interface HomeViewProps {
  disabled?: boolean;
  onError?: (message: string) => void;
  onNavigate: (view: SessionView) => void;
}

/** Arena360 in-session Home: branded hero + Quick Launch from the games catalog. */
export function HomeView({ disabled, onError, onNavigate }: HomeViewProps) {
  const { items: games } = useCatalog(fetchActiveGames);
  const { launchingKey, isLaunchable, launch } = useLauncher(Boolean(disabled), onError);
  const featured = games[0];

  return (
    <div>
      <section className="a360-hero a360-hero--branded">
        <div className="a360-hero-content">
          <div className="a360-chip-row">
            <span className="a360-chip a360-chip-primary">This Station</span>
          </div>
          <h1 className="a360-hero-title">Ready to play</h1>
          <p className="a360-hero-desc">Launch the games and tools installed on this PC.</p>
          <div className="a360-hero-actions">
            {featured ? (
              <button
                type="button"
                className="primary-glow-btn a360-btn-lg"
                disabled={disabled || launchingKey !== null}
                onClick={() => void launch(featured)}
              >
                <span className="material-symbols-outlined is-filled">play_arrow</span>
                {launchingKey === featured.id ? 'Launching…' : `Play ${featured.name}`}
              </button>
            ) : null}
            <button
              type="button"
              className="a360-btn-ghost a360-btn-lg"
              onClick={() => onNavigate('library')}
            >
              <span className="material-symbols-outlined">grid_view</span>
              Browse Library
            </button>
          </div>
        </div>
      </section>

      <section className="a360-section a360-section--pull">
        {games.length > 0 ? (
          <div className="a360-carousel no-scrollbar">
            {games.map((game) => (
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
        ) : (
          <p className="a360-empty">No games installed. Ask staff to set up this station.</p>
        )}
      </section>
    </div>
  );
}
