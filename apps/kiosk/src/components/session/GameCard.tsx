import type { KioskGame } from '../../lib/games';

interface GameCardProps {
  game: KioskGame;
  /** Whether this game resolves to an installed allow-list entry. */
  launchable: boolean;
  launching: boolean;
  disabled?: boolean;
  onLaunch: () => void;
}

/** A 3:4 glass game poster with hover video preview (Arena360 library/home). */
export function GameCard({ game, launchable, launching, disabled, onLaunch }: GameCardProps) {
  const clickable = launchable && !disabled;
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
          className="a360-game-card-video"
          src={game.videoUrl}
          autoPlay
          loop
          muted
          playsInline
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
