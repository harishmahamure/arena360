import { useEffect, useRef } from 'react';
import type { LaunchEntry } from '../../lib/allowList';

interface HeroGameCarouselProps {
  games: LaunchEntry[];
  selectedIndex: number;
  onSelect: (index: number) => void;
  onAllGames: () => void;
}

/** Horizontal game picker for the cinematic home hero (active tile scales up). */
export function HeroGameCarousel({
  games,
  selectedIndex,
  onSelect,
  onAllGames,
}: HeroGameCarouselProps) {
  const tileRefs = useRef<(HTMLButtonElement | null)[]>([]);

  useEffect(() => {
    const tile = tileRefs.current[selectedIndex];
    tile?.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'center' });
  }, [selectedIndex]);

  if (games.length === 0) return null;

  return (
    <div className="a360-hero-carousel-wrap">
      <div className="a360-hero-carousel no-scrollbar" role="listbox" aria-label="Featured games">
        {games.map((game, index) => {
          const active = index === selectedIndex;
          return (
            <button
              key={game.id}
              type="button"
              ref={(el) => {
                tileRefs.current[index] = el;
              }}
              role="option"
              aria-selected={active}
              className={`a360-hero-tile${active ? ' is-active' : ''}`}
              onClick={() => onSelect(index)}
            >
              {game.thumbnailUrl ? (
                <img className="a360-hero-tile-thumb" src={game.thumbnailUrl} alt="" />
              ) : (
                <span className="a360-hero-tile-fallback">
                  <span className="material-symbols-outlined">sports_esports</span>
                </span>
              )}
              <span className="a360-hero-tile-grad" aria-hidden="true" />
              {game.logoUrl ? (
                <img className="a360-hero-tile-logo" src={game.logoUrl} alt="" />
              ) : (
                <span className="a360-hero-tile-name">{game.name}</span>
              )}
            </button>
          );
        })}
        <button type="button" className="a360-hero-tile a360-hero-tile--all" onClick={onAllGames}>
          All games
        </button>
      </div>
    </div>
  );
}
