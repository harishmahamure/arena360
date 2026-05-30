import { useEffect, useMemo, useState } from 'react';
import { allowListPaths, type LaunchEntry, loadLaunchEntries } from '../lib/allowList';
import { fetchActiveGames, type Game } from '../lib/games';
import { launchAllowed } from '../lib/tauriCommands';
import { IconFallback } from './IconFallback';

interface LauncherGridProps {
  /** Disable launching (e.g. during force-end grace). */
  disabled?: boolean;
  onError?: (message: string) => void;
}

/** Resolve the allow-list entry a catalog game maps to (ADR-0019): by explicit
 * `launchRef` (entry id or name) or by matching display names. */
function matchEntry(game: Game, entries: LaunchEntry[]): LaunchEntry | undefined {
  const ref = game.launchRef?.toLowerCase();
  if (ref) {
    const byId = entries.find((e) => e.id.toLowerCase() === ref);
    if (byId) return byId;
    const byName = entries.find((e) => e.name.toLowerCase() === ref);
    if (byName) return byName;
  }
  return entries.find((e) => e.name.toLowerCase() === game.name.toLowerCase());
}

/**
 * Player-facing ggLeap-style game grid. Cards render from the DB catalog
 * (thumbnail/logo, DRAFT-0022); launching resolves to the client allow-list
 * (ADR-0019) and calls the native `launch_allowed` guard. Falls back to the raw
 * allow-list tiles when the catalog is empty.
 */
export function LauncherGrid({ disabled, onError }: LauncherGridProps) {
  const entries = useMemo<LaunchEntry[]>(
    () => loadLaunchEntries().filter((e) => e.present !== false),
    [],
  );
  const [games, setGames] = useState<Game[] | null>(null);
  const [launchingKey, setLaunchingKey] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    fetchActiveGames()
      .then((g) => {
        if (active) setGames(g);
      })
      .catch(() => {
        if (active) setGames([]);
      });
    return () => {
      active = false;
    };
  }, []);

  async function launch(key: string, entry: LaunchEntry) {
    if (disabled) return;
    setLaunchingKey(key);
    try {
      await launchAllowed(entry.executablePath, allowListPaths(entries), entry.arguments);
    } catch (e) {
      onError?.(e instanceof Error ? e.message : `Could not launch ${entry.name}`);
    } finally {
      setLaunchingKey(null);
    }
  }

  // Catalog-driven grid (preferred).
  if (games && games.length > 0) {
    return (
      <div className="game-grid">
        {games.map((game) => {
          const entry = matchEntry(game, entries);
          const launchable = Boolean(entry) && !disabled;
          return (
            <button
              key={game.id}
              type="button"
              className="game-card"
              disabled={!launchable || launchingKey !== null}
              onClick={() => entry && void launch(game.id, entry)}
              title={entry ? game.name : `${game.name} (not installed on this station)`}
            >
              {game.thumbnailUrl ? (
                <img className="game-card-thumb" src={game.thumbnailUrl} alt="" />
              ) : (
                <span className="game-card-thumb game-card-thumb-empty">
                  <IconFallback name={game.name} size={56} className="game-card-fallback-icon" />
                </span>
              )}
              <span className="game-card-overlay">
                {game.logoUrl ? (
                  <img className="game-card-logo" src={game.logoUrl} alt="" />
                ) : (
                  <span className="game-card-name">{game.name}</span>
                )}
              </span>
              {launchingKey === game.id ? (
                <span className="game-card-status">Launching…</span>
              ) : null}
            </button>
          );
        })}
      </div>
    );
  }

  // Fallback: render the raw allow-list when no catalog exists yet.
  if (entries.length === 0) {
    return <p className="hint">No games configured. Ask staff to set up this station.</p>;
  }

  return (
    <div className="launcher-grid">
      {entries.map((entry) => (
        <button
          key={entry.id}
          type="button"
          className="launcher-tile"
          disabled={disabled || launchingKey !== null}
          onClick={() => void launch(entry.id, entry)}
        >
          <IconFallback name={entry.name} size={36} className="launcher-tile-icon" />
          <span className="launcher-tile-name">{entry.name}</span>
          {launchingKey === entry.id ? (
            <span className="launcher-tile-status">Launching…</span>
          ) : null}
        </button>
      ))}
    </div>
  );
}
