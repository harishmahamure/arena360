import { useEffect, useState } from 'react';
import { type LaunchCategory, type LaunchEntry, loadLaunchEntries } from '../lib/allowList';
import { blankGame, fetchCatalog, type KioskGame, removeGame, upsertGame } from '../lib/games';
import { resolveEntry } from '../lib/launch';
import { GalleryPicker } from './GalleryPicker';

const CATEGORY_OPTIONS: { value: LaunchCategory; label: string }[] = [
  { value: 'game', label: 'Game' },
  { value: 'launcher', label: 'Launcher' },
  { value: 'util', label: 'Utility' },
];

type MediaField = 'logoUrl' | 'thumbnailUrl' | 'videoUrl';

/** Setup-mode editor for the games catalog (ADR-0019). */
export function GameCatalogEditor() {
  const [games, setGames] = useState<KioskGame[]>([]);
  const [entries] = useState<LaunchEntry[]>(() => loadLaunchEntries());
  const [draft, setDraft] = useState<KioskGame | null>(null);
  const [picker, setPicker] = useState<{ field: MediaField } | null>(null);

  useEffect(() => {
    void fetchCatalog().then(setGames);
  }, []);

  function setField<K extends keyof KioskGame>(key: K, value: KioskGame[K]) {
    setDraft((d) => (d ? { ...d, [key]: value } : d));
  }

  function save() {
    if (!draft) return;
    const name = draft.name.trim();
    if (!name) return;
    setGames(upsertGame({ ...draft, name }));
    setDraft(null);
  }

  function remove(id: string) {
    setGames(removeGame(id));
    if (draft?.id === id) setDraft(null);
  }

  const installed = (g: KioskGame) => Boolean(resolveEntry(g, entries));
  const pickerKind = picker?.field === 'videoUrl' ? 'video' : 'image';

  return (
    <div className="allow-list-editor">
      <div className="allow-list-section">
        <div className="allow-list-header">
          <h2>Games catalog ({games.length})</h2>
          <button type="button" onClick={() => setDraft(blankGame())}>
            Add game
          </button>
        </div>

        {games.length === 0 ? (
          <p className="hint">No games yet. Add one — players see it on Home and in the Library.</p>
        ) : (
          <ul className="catalog-list">
            {games.map((game) => (
              <li key={game.id} className={game.isActive === false ? 'is-inactive' : undefined}>
                <span className="catalog-art">
                  {game.thumbnailUrl || game.logoUrl ? (
                    <img src={game.thumbnailUrl ?? game.logoUrl ?? ''} alt="" />
                  ) : (
                    <span className="material-symbols-outlined">sports_esports</span>
                  )}
                </span>
                <span className="catalog-info">
                  <span className="catalog-name">{game.name}</span>
                  <span className="catalog-meta">
                    <span className="catalog-tag">{game.category ?? 'game'}</span>
                    <span className={installed(game) ? 'catalog-ok' : 'catalog-warn'}>
                      {installed(game) ? 'Installed' : 'Not installed'}
                    </span>
                    {game.isActive === false ? <span className="catalog-warn">Hidden</span> : null}
                  </span>
                </span>
                <span className="catalog-actions">
                  <button type="button" className="secondary" onClick={() => setDraft({ ...game })}>
                    Edit
                  </button>
                  <button type="button" className="danger" onClick={() => remove(game.id)}>
                    Remove
                  </button>
                </span>
              </li>
            ))}
          </ul>
        )}
      </div>

      {draft ? (
        <div className="allow-list-section catalog-form">
          <h2>{draft.id ? 'Edit game' : 'Add game'}</h2>
          <div className="catalog-fields">
            <label className="catalog-field">
              Name
              <input
                value={draft.name}
                onChange={(e) => setField('name', e.target.value)}
                placeholder="Display name"
              />
            </label>

            <label className="catalog-field">
              Launches (allow-list app)
              <input
                value={draft.launchRef ?? ''}
                list="catalog-launchrefs"
                onChange={(e) => setField('launchRef', e.target.value)}
                placeholder="Matches an installed app name"
              />
              <datalist id="catalog-launchrefs">
                {entries.map((e) => (
                  <option key={e.id} value={e.name} />
                ))}
              </datalist>
            </label>

            <label className="catalog-field">
              Section
              <select
                value={draft.category ?? 'game'}
                onChange={(e) => setField('category', e.target.value as LaunchCategory)}
              >
                {CATEGORY_OPTIONS.map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                  </option>
                ))}
              </select>
            </label>

            <label className="catalog-field">
              Genre
              <input
                value={draft.genre ?? ''}
                onChange={(e) => setField('genre', e.target.value)}
                placeholder="e.g. Tactical Shooter"
              />
            </label>

            <label className="catalog-field catalog-field--wide">
              Description
              <input
                value={draft.description ?? ''}
                onChange={(e) => setField('description', e.target.value)}
                placeholder="Short blurb shown on the card"
              />
            </label>

            <label className="catalog-field catalog-field--narrow">
              Sort order
              <input
                type="number"
                value={draft.sortOrder ?? 0}
                onChange={(e) => setField('sortOrder', Number(e.target.value) || 0)}
              />
            </label>

            <label className="catalog-field catalog-field--check">
              <input
                type="checkbox"
                checked={draft.isActive !== false}
                onChange={(e) => setField('isActive', e.target.checked)}
              />
              Visible to players
            </label>
          </div>

          <div className="catalog-media">
            <MediaSlot
              label="Logo"
              kind="image"
              url={draft.logoUrl ?? null}
              onPick={() => setPicker({ field: 'logoUrl' })}
              onClear={() => setField('logoUrl', null)}
            />
            <MediaSlot
              label="Thumbnail"
              kind="image"
              url={draft.thumbnailUrl ?? null}
              onPick={() => setPicker({ field: 'thumbnailUrl' })}
              onClear={() => setField('thumbnailUrl', null)}
            />
            <MediaSlot
              label="Preview video"
              kind="video"
              url={draft.videoUrl ?? null}
              onPick={() => setPicker({ field: 'videoUrl' })}
              onClear={() => setField('videoUrl', null)}
            />
          </div>

          <div className="catalog-form-actions">
            <button type="button" className="secondary" onClick={() => setDraft(null)}>
              Cancel
            </button>
            <button type="button" onClick={save} disabled={!draft.name.trim()}>
              {draft.id ? 'Save changes' : 'Add to catalog'}
            </button>
          </div>
        </div>
      ) : null}

      {picker ? (
        <GalleryPicker
          kind={pickerKind}
          value={draft?.[picker.field] ?? null}
          onSelect={(url) => setField(picker.field, url)}
          onClose={() => setPicker(null)}
        />
      ) : null}
    </div>
  );
}

interface MediaSlotProps {
  label: string;
  kind: 'image' | 'video';
  url: string | null;
  onPick: () => void;
  onClear: () => void;
}

function MediaSlot({ label, kind, url, onPick, onClear }: MediaSlotProps) {
  return (
    <div className="catalog-media-slot">
      <span className="catalog-media-label">{label}</span>
      <button type="button" className="catalog-media-preview" onClick={onPick}>
        {url ? (
          kind === 'video' ? (
            <video src={url} muted playsInline preload="metadata" />
          ) : (
            <img src={url} alt="" />
          )
        ) : (
          <span className="catalog-media-empty">
            <span className="material-symbols-outlined">add_photo_alternate</span>
            Choose…
          </span>
        )}
      </button>
      {url ? (
        <button type="button" className="link danger" onClick={onClear}>
          Clear
        </button>
      ) : null}
    </div>
  );
}
