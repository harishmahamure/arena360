import { useEffect, useRef } from 'react';
import type { KioskGame } from '../../lib/games';

interface GameCardProps {
  game: KioskGame;
  /** Whether this game resolves to an installed allow-list entry. */
  launchable: boolean;
  launching: boolean;
  disabled?: boolean;
  onLaunch: () => void;
}

/** A 3:4 glass game poster with thumbnail background and autoplay video overlay. */
export function GameCard({ game, launchable, launching, disabled, onLaunch }: GameCardProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const clickable = launchable && !disabled;

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    void video.play().catch(() => {
      // Autoplay may be blocked until user interaction; muted + playsInline usually succeeds.
    });
  }, [game.videoUrl]);

  return (
    <button
      type="button"
      className="a360-game-card"
      disabled={!clickable || launching}
      onClick={onLaunch}
      title={launchable ? game.name : `${game.name} (not installed on this station)`}
    >
      {game.thumbnailUrl ? (
        <img className="a360-game-card-thumb" src={game.thumbnailUrl} alt="" />
      ) : (
        <span className="a360-game-card-empty">
          <span className="material-symbols-outlined">sports_esports</span>
        </span>
      )}

      {game.videoUrl ? (
        <video
          ref={videoRef}
          className="a360-game-card-video"
          src={game.videoUrl}
          autoPlay
          loop
          muted
          playsInline
          preload="auto"
        />
      ) : null}

      <span className="a360-game-card-grad" />

      <span className={`a360-game-card-badge ${launchable ? 'is-ready' : 'is-unavailable'}`}>
        {launchable ? 'Ready' : 'Not installed'}
      </span>

      <span className="a360-game-card-info">
        <span className="a360-game-card-name">{game.name}</span>
        {game.genre ? (
          <span className="a360-game-card-genre">
            <span className="material-symbols-outlined">bolt</span>
            {game.genre}
          </span>
        ) : null}
      </span>

      {launching ? <span className="a360-game-card-status">Launching…</span> : null}
    </button>
  );
}
