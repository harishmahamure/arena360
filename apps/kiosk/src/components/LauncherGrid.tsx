import { useEffect, useMemo, useState } from 'react';
import {
  allowListPaths,
  entryCategory,
  type LaunchCategory,
  type LaunchEntry,
  loadLaunchEntries,
} from '../lib/allowList';
import { fetchActiveGames, type Game } from '../lib/games';
import { launchAllowed } from '../lib/tauriCommands';
import { IconFallback } from './IconFallback';

interface LauncherGridProps {
  /** Disable launching (e.g. during force-end grace). */
  disabled?: boolean;
  onError?: (message: string) => void;
}

/** A launchable item rendered as a tile in one of the home sections. */
interface Tile {
  key: string;
  name: string;
  thumbnailUrl?: string | null;
  logoUrl?: string | null;
  /** The allow-list entry to launch; absent means "not installed here". */
  entry?: LaunchEntry;
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
 * Player-facing home: launchable apps grouped into Games / Launchers / Utils
 * (ggCircuit-style). Games come from the DB catalog (thumbnail/logo, DRAFT-0022)
 * plus any `game`-category allow-list entries with no catalog art; Launchers and
 * Utils come from the local allow-list category (ADR-0019). Launching resolves
 * to the native `launch_allowed` guard.
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

  const sections = useMemo(() => buildSections(games ?? [], entries), [games, entries]);

  const hasAnything =
    sections.game.length > 0 || sections.launcher.length > 0 || sections.util.length > 0;

  if (games === null) {
    return <p className="hint">Loading…</p>;
  }

  if (!hasAnything) {
    return <p className="hint">No applications configured. Ask staff to set up this station.</p>;
  }

  function renderTile(tile: Tile) {
    const launchable = Boolean(tile.entry) && !disabled;
    return (
      <button
        key={tile.key}
        type="button"
        className="game-card"
        disabled={!launchable || launchingKey !== null}
        onClick={() => tile.entry && void launch(tile.key, tile.entry)}
        title={tile.entry ? tile.name : `${tile.name} (not installed on this station)`}
      >
        {tile.thumbnailUrl ? (
          <img className="game-card-thumb" src={tile.thumbnailUrl} alt="" />
        ) : (
          <span className="game-card-thumb game-card-thumb-empty">
            <IconFallback name={tile.name} size={56} className="game-card-fallback-icon" />
          </span>
        )}
        <span className="game-card-overlay">
          {tile.logoUrl ? (
            <img className="game-card-logo" src={tile.logoUrl} alt="" />
          ) : (
            <span className="game-card-name">{tile.name}</span>
          )}
        </span>
        {launchingKey === tile.key ? <span className="game-card-status">Launching…</span> : null}
      </button>
    );
  }

  return (
    <div className="launcher-sections">
      {SECTION_ORDER.map(({ key, title }) =>
        sections[key].length > 0 ? (
          <section key={key} className="launcher-section">
            <h2 className="launcher-section-title">{title}</h2>
            <div className="game-grid">{sections[key].map(renderTile)}</div>
          </section>
        ) : null,
      )}
    </div>
  );
}

const SECTION_ORDER: { key: LaunchCategory; title: string }[] = [
  { key: 'game', title: 'Games' },
  { key: 'launcher', title: 'Launchers' },
  { key: 'util', title: 'Utilities' },
];

/** Group catalog games and allow-list entries into the three home sections. */
function buildSections(games: Game[], entries: LaunchEntry[]): Record<LaunchCategory, Tile[]> {
  const entryArt = new Map<string, Game>();
  for (const game of games) {
    const entry = matchEntry(game, entries);
    if (entry) entryArt.set(entry.id, game);
  }
  const matchedIds = new Set(entryArt.keys());

  const gameTiles: Tile[] = games.map((game) => ({
    key: game.id,
    name: game.name,
    thumbnailUrl: game.thumbnailUrl,
    logoUrl: game.logoUrl,
    entry: matchEntry(game, entries),
  }));

  const toTile = (entry: LaunchEntry): Tile => {
    const art = entryArt.get(entry.id);
    return {
      key: entry.id,
      name: entry.name,
      thumbnailUrl: art?.thumbnailUrl,
      logoUrl: art?.logoUrl,
      entry,
    };
  };

  // Standalone `game`-category entries that no catalog game already represents.
  for (const entry of entries) {
    if (entryCategory(entry) === 'game' && !matchedIds.has(entry.id)) {
      gameTiles.push(toTile(entry));
    }
  }

  return {
    game: gameTiles,
    launcher: entries.filter((e) => entryCategory(e) === 'launcher').map(toTile),
    util: entries.filter((e) => entryCategory(e) === 'util').map(toTile),
  };
}
