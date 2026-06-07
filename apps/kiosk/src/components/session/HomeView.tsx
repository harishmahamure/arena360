import { useEffect, useState } from 'react';
import { fetchGames } from '../../lib/allowList';
import { LOGIN_BACKGROUND_VIDEO_URL } from '../../lib/config';
import { HeroGameCarousel } from './HeroGameCarousel';
import type { SessionView } from './SessionNav';
import { useAllowList } from './useAllowList';
import { useLauncher } from './useLauncher';

interface HomeViewProps {
  disabled?: boolean;
  onError?: (message: string) => void;
  onNavigate: (view: SessionView) => void;
  onSearchLibrary?: (query: string) => void;
  onLaunched?: () => void;
}

function tryAutoplay(video: HTMLVideoElement) {
  void Promise.resolve(video.play()).catch(() => {});
}

export function HomeView({
  disabled,
  onError,
  onNavigate,
  onSearchLibrary,
  onLaunched,
}: HomeViewProps) {
  const { items: games } = useAllowList(fetchGames);
  const { launchingKey, isLaunchable, launch } = useLauncher(
    Boolean(disabled),
    onError,
    onLaunched,
  );
  const [selectedIndex, setSelectedIndex] = useState(0);
  const [homeQuery, setHomeQuery] = useState('');

  useEffect(() => {
    if (selectedIndex >= games.length) setSelectedIndex(0);
  }, [games.length, selectedIndex]);

  const featured = games[selectedIndex];

  const canPlay = featured ? isLaunchable(featured) : false;
  const isLaunching = featured ? launchingKey === featured.id : false;
  const background = featured?.videoUrl ?? LOGIN_BACKGROUND_VIDEO_URL;

  function submitLibrarySearch(e: React.FormEvent) {
    e.preventDefault();
    const q = homeQuery.trim();
    if (!q) {
      onNavigate('library');
      return;
    }
    onSearchLibrary?.(q);
    onNavigate('library');
  }

  return (
    <div className="a360-home">
      <section className="a360-hero a360-hero--cinematic">
        <div className="a360-hero-media" aria-hidden="true">
          <video
            key={background}
            src={background}
            autoPlay
            loop
            muted
            playsInline
            preload="auto"
            poster={featured?.thumbnailUrl ?? undefined}
            onLoadedData={(e) => tryAutoplay(e.currentTarget)}
          />
        </div>
        <div className="a360-hero-scrim a360-hero-scrim--cinematic" />

        <div className="a360-hero-content">
          {games.length > 0 ? (
            <form className="a360-home-search" onSubmit={submitLibrarySearch}>
              <span className="material-symbols-outlined" aria-hidden="true">
                search
              </span>
              <input
                type="search"
                value={homeQuery}
                placeholder="Search your library…"
                aria-label="Search games in library"
                onChange={(e) => setHomeQuery(e.target.value)}
              />
              <button type="submit" className="a360-btn-ghost a360-home-search-btn">
                Browse
              </button>
            </form>
          ) : null}

          <HeroGameCarousel
            games={games}
            selectedIndex={selectedIndex}
            onSelect={setSelectedIndex}
            onAllGames={() => onNavigate('library')}
          />

          <div className="a360-hero-bottom">
            <h1 className="a360-hero-title">{featured?.name ?? 'Ready to play'}</h1>
            <div className="a360-hero-actions">
              <button
                type="button"
                className="primary-glow-btn a360-btn-lg"
                disabled={disabled || !canPlay || isLaunching}
                onClick={() => featured && void launch(featured)}
              >
                <span className="material-symbols-outlined is-filled">play_arrow</span>
                {isLaunching ? 'Launching…' : 'Play now!'}
              </button>
              <button
                type="button"
                className="a360-btn-ghost a360-btn-lg"
                onClick={() => onNavigate('library')}
              >
                <span className="material-symbols-outlined">grid_view</span>
                Explore Library
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
}
