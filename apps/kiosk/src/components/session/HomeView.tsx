import { useEffect, useMemo, useState } from 'react';
import { fetchGames } from '../../lib/allowList';
import { LOGIN_BACKGROUND_VIDEO_URL } from '../../lib/config';
import { GameCard } from './GameCard';
import { HeroGameCarousel } from './HeroGameCarousel';
import type { SessionView } from './SessionNav';
import { useAllowList } from './useAllowList';
import { useLauncher } from './useLauncher';

const QUICK_LAUNCH_LIMIT = 12;

interface HomeViewProps {
  deviceName?: string | null;
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
  deviceName,
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

  useEffect(() => {
    if (selectedIndex >= games.length) setSelectedIndex(0);
  }, [games.length, selectedIndex]);

  const featured = games[selectedIndex];
  const quickLaunch = useMemo(() => games.slice(0, QUICK_LAUNCH_LIMIT), [games]);

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
            games={games}
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

      {quickLaunch.length > 0 ? (
        <section className="a360-quick-launch">
          <div className="a360-section-head">
            <div>
              <h2 className="a360-section-title">
                <span className="material-symbols-outlined">rocket_launch</span>
                Quick Launch
              </h2>
              <p className="a360-section-sub">Jump back into your recently allowed adventures</p>
            </div>
            <button type="button" className="a360-link-btn" onClick={() => onNavigate('library')}>
              View all games
              <span className="material-symbols-outlined">arrow_forward</span>
            </button>
          </div>
          <div className="a360-carousel no-scrollbar">
            {quickLaunch.map((entry) => (
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
        </section>
      ) : null}

      <footer className="a360-home-footer">
        <div>
          <span className="a360-home-footer-brand">Arena360</span>
          <p className="a360-home-footer-meta">Experience the future.</p>
        </div>
        {deviceName ? <span className="a360-home-footer-station">{deviceName}</span> : null}
      </footer>
    </div>
  );
}
