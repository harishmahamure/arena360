import { useEffect, useMemo, useState } from 'react';
import { LOGIN_BACKGROUND_VIDEO_URL } from '../../lib/config';
import { fetchActiveGames, type KioskGame } from '../../lib/games';
import { HeroGameCarousel } from './HeroGameCarousel';
import type { SessionView } from './SessionNav';
import { useCatalog } from './useCatalog';
import { useLauncher } from './useLauncher';

interface HomeViewProps {
  disabled?: boolean;
  onError?: (message: string) => void;
  onNavigate: (view: SessionView) => void;
}

function tryAutoplay(video: HTMLVideoElement) {
  void Promise.resolve(video.play()).catch(() => {});
}

export function HomeView({ disabled, onError, onNavigate }: HomeViewProps) {
  const { items: games } = useCatalog(fetchActiveGames);
  const { launchingKey, isLaunchable, launch } = useLauncher(Boolean(disabled), onError);
  const [selectedIndex, setSelectedIndex] = useState(0);

  useEffect(() => {
    if (selectedIndex >= games.length) setSelectedIndex(0);
  }, [games.length, selectedIndex]);

  const featured = games[selectedIndex];

  const canPlay = featured ? isLaunchable(featured) : false;
  const isLaunching = featured ? launchingKey === featured.id : false;
  const background = featured?.videoUrl ?? LOGIN_BACKGROUND_VIDEO_URL;

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
