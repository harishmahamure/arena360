import { memo, useEffect, useMemo, useState } from 'react';
import { fetchGames } from '../../lib/allowList';
import { LOGIN_BACKGROUND_VIDEO_URL } from '../../lib/config';
import { HeroGameCarousel } from './HeroGameCarousel';
import type { SessionView } from './SessionNav';
import { useAllowList } from './useAllowList';
import { useLauncher } from './useLauncher';

const HERO_CAROUSEL_LIMIT = 4;

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

export const HomeView = memo(function HomeView({
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
  const heroGames = useMemo(() => games.slice(0, HERO_CAROUSEL_LIMIT), [games]);

  useEffect(() => {
    if (selectedIndex >= heroGames.length) setSelectedIndex(0);
  }, [heroGames.length, selectedIndex]);

  const featured = heroGames[selectedIndex];

  const canPlay = featured ? isLaunchable(featured) : false;
  const isLaunching = featured ? launchingKey === featured.id : false;
  const background = featured?.videoUrl ?? LOGIN_BACKGROUND_VIDEO_URL;

  function openGameDetails() {
    if (featured?.name) onSearchLibrary?.(featured.name);
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
          <HeroGameCarousel
            games={heroGames}
            selectedIndex={selectedIndex}
            onSelect={setSelectedIndex}
            onAllGames={() => onNavigate('library')}
          />

          <div className="a360-hero-bottom">
            {featured?.genre ? (
              <div className="a360-chip-row">
                <span className="a360-chip a360-chip-primary">Featured</span>
                <span className="a360-chip a360-chip-glass">{featured.genre}</span>
              </div>
            ) : null}
            <h1 className="a360-hero-title">{featured?.name ?? 'Ready to play'}</h1>
            {featured?.description ? (
              <p className="a360-hero-description">{featured.description}</p>
            ) : null}
            <div className="a360-hero-actions">
              <button
                type="button"
                className="primary-glow-btn a360-btn-lg"
                disabled={disabled || !canPlay || isLaunching}
                onClick={() => featured && void launch(featured)}
              >
                <span className="material-symbols-outlined is-filled">play_arrow</span>
                {isLaunching ? 'Launching…' : 'Play Now'}
              </button>
              <button
                type="button"
                className="a360-btn-ghost a360-btn-lg"
                onClick={openGameDetails}
              >
                <span className="material-symbols-outlined">info</span>
                Game details
              </button>
            </div>
          </div>
        </div>
      </section>
    </div>
  );
});
