import { useCallback, useEffect, useState } from 'react';
import { addGalleryItem, type GalleryItem, type GalleryKind, loadGallery } from '../lib/gallery';
import { pickMediaFile } from '../lib/tauriCommands';

interface GalleryPickerProps {
  /** Restrict the picker to one media kind. */
  kind: GalleryKind;
  /** Currently selected asset url, if any. */
  value?: string | null;
  /** Called with the chosen url, or null when cleared. */
  onSelect: (url: string | null) => void;
  onClose: () => void;
}

/** Modal grid for picking one gallery asset (and adding new ones inline). */
export function GalleryPicker({ kind, value, onSelect, onClose }: GalleryPickerProps) {
  const [items, setItems] = useState<GalleryItem[]>([]);
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');

  useEffect(() => {
    void loadGallery().then(setItems);
  }, []);

  const visible = items.filter((i) => i.kind === kind);

  const add = useCallback(
    (assetUrl: string, label: string) => {
      const next = addGalleryItem({ kind, name: label, url: assetUrl });
      setItems(next);
      return next.find((i) => i.url === assetUrl)?.url ?? assetUrl;
    },
    [kind],
  );

  function addFromForm(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = url.trim();
    if (!trimmed) return;
    add(trimmed, name.trim());
    setName('');
    setUrl('');
  }

  async function browse() {
    const src = await pickMediaFile(kind);
    if (!src) return;
    const chosen = add(src, name.trim() || `${kind} ${visible.length + 1}`);
    onSelect(chosen);
    onClose();
  }

  return (
    <div className="picker-overlay" role="dialog" aria-modal="true" aria-label={`Select ${kind}`}>
      <div className="picker-modal">
        <header className="picker-head">
          <h3>Select {kind === 'video' ? 'a video' : 'an image'}</h3>
          <button type="button" className="picker-close" aria-label="Close" onClick={onClose}>
            <span className="material-symbols-outlined">close</span>
          </button>
        </header>

        <div className="picker-grid">
          <button
            type="button"
            className={`picker-tile picker-tile--none ${value ? '' : 'is-selected'}`}
            onClick={() => {
              onSelect(null);
              onClose();
            }}
          >
            <span className="material-symbols-outlined">block</span>
            <span>None</span>
          </button>

          {visible.map((item) => (
            <button
              type="button"
              key={item.id}
              className={`picker-tile ${value === item.url ? 'is-selected' : ''}`}
              title={item.name}
              onClick={() => {
                onSelect(item.url);
                onClose();
              }}
            >
              {item.kind === 'video' ? (
                <video src={item.url} muted playsInline preload="metadata" />
              ) : (
                <img src={item.url} alt="" />
              )}
              <span className="picker-tile-name">{item.name}</span>
            </button>
          ))}
        </div>

        <form className="picker-add" onSubmit={addFromForm}>
          <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Asset name" />
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder={
              kind === 'video'
                ? '/games/videos/clip.mp4 or https://…'
                : '/games/images/art.png or https://…'
            }
          />
          <button type="button" className="secondary" onClick={() => void browse()}>
            Browse…
          </button>
          <button type="submit">Add to gallery</button>
        </form>
      </div>
    </div>
  );
}
