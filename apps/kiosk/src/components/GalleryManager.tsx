import { useEffect, useState } from 'react';
import {
  addGalleryItem,
  type GalleryItem,
  type GalleryKind,
  loadGallery,
  removeGalleryItem,
} from '../lib/gallery';
import { pickMediaFile } from '../lib/tauriCommands';

/** Setup-mode gallery manager: curate the logos/posters/videos catalog games pick from. */
export function GalleryManager() {
  const [items, setItems] = useState<GalleryItem[]>([]);
  const [name, setName] = useState('');
  const [url, setUrl] = useState('');
  const [kind, setKind] = useState<GalleryKind>('image');

  useEffect(() => {
    void loadGallery().then(setItems);
  }, []);

  function add(e: React.FormEvent) {
    e.preventDefault();
    if (!url.trim()) return;
    setItems(addGalleryItem({ kind, name: name.trim(), url: url.trim() }));
    setName('');
    setUrl('');
  }

  async function browse() {
    const src = await pickMediaFile(kind);
    if (!src) return;
    setItems(addGalleryItem({ kind, name: name.trim() || `${kind} asset`, url: src }));
    setName('');
  }

  return (
    <div className="allow-list-editor">
      <div className="allow-list-section">
        <div className="allow-list-header">
          <h2>Media gallery ({items.length})</h2>
        </div>
        {items.length === 0 ? (
          <p className="hint">No media yet. Add logos, posters or preview videos below.</p>
        ) : (
          <div className="gallery-grid">
            {items.map((item) => (
              <figure key={item.id} className="gallery-item">
                <div className="gallery-thumb">
                  {item.kind === 'video' ? (
                    <video src={item.url} muted playsInline preload="metadata" />
                  ) : (
                    <img src={item.url} alt="" />
                  )}
                  <span className="gallery-kind">{item.kind}</span>
                </div>
                <figcaption className="gallery-caption">{item.name}</figcaption>
                <button
                  type="button"
                  className="danger gallery-remove"
                  onClick={() => setItems(removeGalleryItem(item.id))}
                >
                  Remove
                </button>
              </figure>
            ))}
          </div>
        )}
      </div>

      <div className="allow-list-section">
        <h2>Add media</h2>
        <p className="hint">Browse for a file on this PC, or paste a served path / URL.</p>
        <form onSubmit={add} className="allow-list-manual">
          <select
            value={kind}
            aria-label="Media kind"
            onChange={(e) => setKind(e.target.value as GalleryKind)}
          >
            <option value="image">Image</option>
            <option value="video">Video</option>
          </select>
          <input value={name} onChange={(e) => setName(e.target.value)} placeholder="Asset name" />
          <input
            value={url}
            onChange={(e) => setUrl(e.target.value)}
            placeholder="/games/images/art.png or https://…"
          />
          <button type="button" className="secondary" onClick={() => void browse()}>
            Browse…
          </button>
          <button type="submit">Add</button>
        </form>
      </div>
    </div>
  );
}
