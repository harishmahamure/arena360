import type { LaunchEntry } from '../../lib/allowList';

interface GameCardProps {
  entry: LaunchEntry;
  launchable: boolean;
  launching: boolean;
  disabled?: boolean;
  onLaunch: () => void;
}

/** A 3:4 glass game poster with thumbnail background and autoplay video overlay. */
function tryAutoplay(video: HTMLVideoElement) {
  void Promise.resolve(video.play()).catch(() => {});
}

export function GameCard({ entry, launchable, launching, disabled, onLaunch }: GameCardProps) {
  const clickable = launchable && !disabled;

  return (
    <button
      type="button"
      className="a360-game-card"
      disabled={!clickable || launching}
      onClick={onLaunch}
      title={launchable ? entry.name : `${entry.name} (executable missing)`}
    >
      {entry.thumbnailUrl ? (
        <img className="a360-game-card-thumb" src={entry.thumbnailUrl} alt="" />
      ) : (
        <span className="a360-game-card-empty">
          <span className="material-symbols-outlined">sports_esports</span>
        </span>
      )}

      {entry.videoUrl ? (
        <video
          key={entry.videoUrl}
          className="a360-game-card-video"
          src={entry.videoUrl}
          autoPlay
          loop
          muted
          playsInline
          preload="auto"
          onLoadedData={(e) => tryAutoplay(e.currentTarget)}
        />
      ) : null}

      <span className="a360-game-card-grad" />

      <span className={`a360-game-card-badge ${launchable ? 'is-ready' : 'is-unavailable'}`}>
        {launchable ? 'Ready' : 'Missing'}
      </span>

      <span className="a360-game-card-info">
        <span className="a360-game-card-name">{entry.name}</span>
        {entry.genre ? (
          <span className="a360-game-card-genre">
            <span className="material-symbols-outlined">bolt</span>
            {entry.genre}
          </span>
        ) : null}
      </span>

      {launching ? <span className="a360-game-card-status">Launching…</span> : null}
    </button>
  );
}
