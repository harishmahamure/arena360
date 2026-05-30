import { useEffect, useState } from 'react';
import type { Game } from '../lib/games';
import { cachedAssetSrc } from '../lib/tauriCommands';

interface BackgroundMediaProps {
  game?: Game;
  showScrim?: boolean;
}

/**
 * Full-screen branded background. Plays the game's looped video (cached on-device
 * per DRAFT-0022) when available, otherwise shows the thumbnail. Renders nothing
 * DB-less so the screen degrades to the plain dark surface.
 */
export function BackgroundMedia({ game, showScrim = true }: BackgroundMediaProps) {
  const [videoSrc, setVideoSrc] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    if (game?.videoUrl) {
      void cachedAssetSrc(game.videoUrl).then((src) => {
        if (active) setVideoSrc(src);
      });
    } else {
      setVideoSrc(null);
    }
    return () => {
      active = false;
    };
  }, [game?.videoUrl]);

  if (videoSrc) {
    return (
      <div className="kiosk-bg" aria-hidden="true">
        <video className="kiosk-bg-video" src={videoSrc} autoPlay loop muted playsInline />
        {showScrim ? <div className="kiosk-bg-scrim" /> : null}
      </div>
    );
  }

  if (game?.thumbnailUrl) {
    return (
      <div className="kiosk-bg" aria-hidden="true">
        <div className="kiosk-bg-image" style={{ backgroundImage: `url(${game.thumbnailUrl})` }} />
        {showScrim ? <div className="kiosk-bg-scrim" /> : null}
      </div>
    );
  }

  return null;
}
