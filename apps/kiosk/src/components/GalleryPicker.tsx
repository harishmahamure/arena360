import { useEffect, useState } from 'react';
import { type GalleryItem, type GalleryKind, refreshGallery } from '../lib/gallery';

interface GalleryPickerProps {
  kind: GalleryKind;
  value?: string | null;
  onSelect: (url: string | null) => void;
  onClose: () => void;
}

/** Read-only modal grid for picking one asset from the CDN-hosted gallery. */
export function GalleryPicker({ kind, value, onSelect, onClose }: GalleryPickerProps) {
  const [items, setItems] = useState<GalleryItem[]>([]);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    void refreshGallery()
      .then(setItems)
      .finally(() => setLoading(false));
  }, []);

  const visible = items.filter((i) => i.kind === kind);

  return (
    <div className="picker-overlay" role="dialog" aria-modal="true" aria-label={`Select ${kind}`}>
      <div className="picker-modal">
        <header className="picker-head">
          <h3>Select {kind === 'video' ? 'a video' : 'an image'}</h3>
          <button type="button" className="picker-close" aria-label="Close" onClick={onClose}>
            <span className="material-symbols-outlined">close</span>
          </button>
        </header>

        <p className="hint picker-hint">
          Gallery is managed centrally on CDN. Updates appear here on refresh.
        </p>

        {loading ? (
          <p className="meta">Loading gallery…</p>
        ) : visible.length === 0 ? (
          <p className="hint">No {kind} assets in the gallery yet.</p>
        ) : (
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
        )}
      </div>
    </div>
  );
}
