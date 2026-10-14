import { useState } from 'react';
import type { LaunchEntry } from '../../lib/allowList';
import { DecorativeVideo } from '../DecorativeVideo';

interface GameCardProps {
  entry: LaunchEntry;
  launchable: boolean;
  launching: boolean;
  disabled?: boolean;
  onLaunch: () => void;
}

/** A 3:4 glass game poster; load its preview only while hovered or focused. */
export function GameCard({ entry, launchable, launching, disabled, onLaunch }: GameCardProps) {
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const clickable = launchable && !disabled;
  const statusLabel = launchable ? 'Ready to launch' : 'Executable missing';

  return (
    <button
      type="button"
      className="a360-game-card"
      disabled={!clickable || launching}
      onClick={onLaunch}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      aria-label={`${entry.name}. ${statusLabel}`}
      title={launchable ? entry.name : `${entry.name} (executable missing)`}
    >
      {entry.thumbnailUrl ? (
        <img className="a360-game-card-thumb" src={entry.thumbnailUrl} alt="" />
      ) : (
        <span className="a360-game-card-empty">
          <span className="material-symbols-outlined">sports_esports</span>
        </span>
      )}

      {entry.videoUrl && (hovered || focused) && clickable && !launching ? (
        <DecorativeVideo
          key={entry.videoUrl}
          className="a360-game-card-video"
          src={entry.videoUrl}
          poster={entry.thumbnailUrl ?? undefined}
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
