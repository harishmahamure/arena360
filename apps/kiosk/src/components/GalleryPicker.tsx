import { useEffect, useState } from 'react';
import {
  type GalleryItem,
  type GalleryKind,
  galleryMediaUrl,
  refreshGallery,
} from '../lib/gallery';

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
  const [error, setError] = useState<string | null>(null);
  const [cacheNonce] = useState(() => Date.now().toString(36));

  useEffect(() => {
    void refreshGallery()
      .then(setItems)
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : 'Gallery unavailable');
      })
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
          Gallery is managed centrally on CDN and requires an internet connection.
        </p>

        {loading ? (
          <p className="meta">Loading gallery…</p>
        ) : error ? (
          <p className="error" role="alert">
            {error}
          </p>
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
                  <video
                    src={galleryMediaUrl(item.url, cacheNonce)}
                    muted
                    playsInline
                    preload="metadata"
                  />
                ) : (
                  <img src={galleryMediaUrl(item.url, cacheNonce)} alt="" />
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
