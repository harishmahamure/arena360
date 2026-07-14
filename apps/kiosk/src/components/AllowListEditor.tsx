import { IconFallback } from '@gaming-cafe/ui/primitives';
import { listen } from '@tauri-apps/api/event';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  addLaunchEntry,
  candidateToEntry,
  categorizeByName,
  entryCategory,
  exportAllowListJson,
  formatLaunchArguments,
  importAllowListJson,
  isTrustedScanSource,
  type LaunchCategory,
  type LaunchEntry,
  type LaunchVia,
  launchViaLabel,
  loadLaunchEntries,
  mergeScanCandidates,
  normalizeLaunchArguments,
  removeLaunchEntry,
  saveLaunchEntries,
  tokenizeArguments,
  updateLaunchEntry,
} from '../lib/allowList';
import type { GalleryItem } from '../lib/gallery';
import { refreshGallery } from '../lib/gallery';
import { applyGalleryMediaToEntries } from '../lib/galleryMatch';
import {
  applyGameBoostSettings,
  loadGameBoostSettings,
  syncGameBoostToNative,
} from '../lib/gameBoost';
import { integerInputProps } from '../lib/inputHints';
import { pickExecutable, type ScanCandidate, scanInstalledSoftware } from '../lib/tauriCommands';
import { GalleryPicker } from './GalleryPicker';

interface ScanProgress {
  scanned: number;
  total: number;
}

const CATEGORY_OPTIONS: { value: LaunchCategory; label: string }[] = [
  { value: 'game', label: 'Game' },
  { value: 'launcher', label: 'Launcher' },
  { value: 'util', label: 'Utility' },
];

type MediaField = 'logoUrl' | 'thumbnailUrl' | 'videoUrl';

function suggestName(path: string): string {
  const base = path.split(/[\\/]/).pop() ?? path;
  const stem = base.replace(/\.(exe|bat|cmd|com|lnk|app)$/i, '');
  return stem
    .replace(/[_-]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
    .replace(/\b\w/g, (c) => c.toUpperCase());
}

export function AllowListEditor({
  onEntriesChange,
}: {
  onEntriesChange?: (count: number) => void;
}) {
  const [entries, setEntries] = useState<LaunchEntry[]>(() => loadLaunchEntries());
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [candidates, setCandidates] = useState<ScanCandidate[]>([]);
  const [scanning, setScanning] = useState(false);
  const [launchingId, setLaunchingId] = useState<string | null>(null);
  const [progress, setProgress] = useState<ScanProgress | null>(null);
  const [manualName, setManualName] = useState('');
  const [manualPath, setManualPath] = useState('');
  const [manualCategory, setManualCategory] = useState<LaunchCategory>('game');
  const [status, setStatus] = useState<string | null>(null);
  const [picker, setPicker] = useState<{ field: MediaField } | null>(null);
  const [gameBoostEnabled, setGameBoostEnabled] = useState(() => loadGameBoostSettings().enabled);
  const [gameBoostAggressive, setGameBoostAggressive] = useState(
    () => loadGameBoostSettings().aggressive,
  );
  const galleryItemsRef = useRef<GalleryItem[] | null>(null);

  const selected = entries.find((e) => e.id === selectedId) ?? null;

  const matchGalleryForAllowList = useCallback(async (forceGallery = false): Promise<number> => {
    if (forceGallery) galleryItemsRef.current = null;
    const cached = galleryItemsRef.current;
    let items = cached;
    if (!items) {
      try {
        items = await refreshGallery();
        galleryItemsRef.current = items;
      } catch {
        return 0;
      }
    }
    if (items.length === 0) return 0;
    const current = loadLaunchEntries();
    const { entries: next, matched } = applyGalleryMediaToEntries(current, items);
    if (matched > 0) {
      saveLaunchEntries(next);
      syncEntries(next);
    }
    return matched;
  }, []);

  useEffect(() => {
    void syncGameBoostToNative();
  }, []);

  useEffect(() => {
    const unlistenProgress = listen<ScanProgress>('scan-progress', (event) => {
      setProgress(event.payload);
    });
    const unlistenStats = listen<{ resolved: number; unresolved: number }>(
      'scan-profile-stats',
      (event) => {
        const { resolved, unresolved } = event.payload;
        if (resolved > 0 || unresolved > 0) {
          setStatus((prev) => {
            const profileNote = `Launcher profiles: ${resolved} resolved, ${unresolved} unresolved`;
            return prev ? `${prev}. ${profileNote}` : profileNote;
          });
        }
      },
    );
    return () => {
      void unlistenProgress.then((fn) => fn());
      void unlistenStats.then((fn) => fn());
    };
  }, []);

  const runScan = useCallback(async () => {
    setScanning(true);
    setProgress(null);
    setStatus(null);
    try {
      const found = await scanInstalledSoftware();
      setCandidates(found);
      galleryItemsRef.current = null;
      const { added, updated, launchersAdded } = mergeScanCandidates(found);
      const mediaMatched = await matchGalleryForAllowList(true);
      if (mediaMatched === 0) syncEntries(loadLaunchEntries());
      const parts: string[] = [];
      if (added > 0) parts.push(`Auto-imported ${added} apps (${launchersAdded} launchers)`);
      if (updated > 0) parts.push(`${updated} updated`);
      if (mediaMatched > 0) parts.push(`Matched gallery media for ${mediaMatched}`);
      if (parts.length > 0) {
        setStatus(parts.join('. '));
      } else if (found.some((c) => c.present && isTrustedScanSource(c.source))) {
        setStatus('Allow-list is up to date.');
      }
    } catch (e) {
      setStatus(e instanceof Error ? e.message : 'Scan failed');
    } finally {
      setScanning(false);
    }
  }, [matchGalleryForAllowList]);

  const initialScanDone = useRef(false);
  useEffect(() => {
    if (initialScanDone.current || entries.length > 0) return;
    initialScanDone.current = true;
    void runScan();
  }, [entries.length, runScan]);

  function syncEntries(next: LaunchEntry[]) {
    setEntries(next);
    onEntriesChange?.(next.length);
    if (selectedId && !next.some((e) => e.id === selectedId)) setSelectedId(null);
  }

  function addCandidate(c: ScanCandidate) {
    const next = addLaunchEntry(candidateToEntry(c));
    syncEntries(next);
    const added = next.find(
      (e) => e.executablePath.toLowerCase() === c.executablePath.toLowerCase(),
    );
    if (added) setSelectedId(added.id);
    void matchGalleryForAllowList();
  }

  function addManual(e: React.FormEvent) {
    e.preventDefault();
    if (!manualName.trim() || !manualPath.trim()) return;
    const next = addLaunchEntry({
      name: manualName.trim(),
      executablePath: manualPath.trim(),
      category: manualCategory,
    });
    syncEntries(next);
    const added = next.find(
      (e) => e.executablePath.toLowerCase() === manualPath.trim().toLowerCase(),
    );
    if (added) setSelectedId(added.id);
    setManualName('');
    setManualPath('');
    setManualCategory('game');
  }

  async function browse() {
    const path = await pickExecutable();
    if (!path) return;
    setManualPath(path);
    if (!manualName.trim()) {
      const name = suggestName(path);
      setManualName(name);
      setManualCategory(categorizeByName(name));
    }
  }

  function remove(id: string) {
    syncEntries(removeLaunchEntry(id));
  }

  function patchSelected(patch: Partial<LaunchEntry>) {
    if (!selectedId) return;
    syncEntries(updateLaunchEntry(selectedId, patch));
  }

  async function testLaunch(entry: LaunchEntry) {
    if (launchingId) return;
    setLaunchingId(entry.id);
    setStatus(null);
    const { launchEntry, launchErrorMessage } = await import('../lib/launch');
    try {
      await launchEntry(entry, entries);
      setStatus(`Launched ${entry.name}`);
    } catch (e) {
      setStatus(launchErrorMessage(e, `Could not launch ${entry.name}`));
    } finally {
      setLaunchingId(null);
    }
  }

  async function browseLauncherExe() {
    const path = await pickExecutable();
    if (!path || !selected?.launchVia) return;
    patchSelected({
      launchVia: { ...selected.launchVia, executablePath: path },
      launchViaAuto: false,
    });
  }

  const alreadyAdded = (path: string) =>
    entries.some((e) => e.executablePath.toLowerCase() === path.toLowerCase());

  const manualCandidates = candidates.filter((c) => !isTrustedScanSource(c.source));

  const pickerKind = picker?.field === 'videoUrl' ? 'video' : 'image';

  return (
    <div className="catalog-editor allow-list-editor">
      <aside className="catalog-editor-list">
        <div className="allow-list-header">
          <h2>Allowed software ({entries.length})</h2>
          <div className="allow-list-header-actions">
            <button type="button" onClick={() => void runScan()} disabled={scanning}>
              {scanning ? 'Scanning…' : 'Scan'}
            </button>
            <button
              type="button"
              className="secondary"
              onClick={() => {
                const blob = new Blob([exportAllowListJson()], { type: 'application/json' });
                const url = URL.createObjectURL(blob);
                const anchor = document.createElement('a');
                anchor.href = url;
                anchor.download = 'arena360-allow-list.json';
                anchor.click();
                URL.revokeObjectURL(url);
                setStatus('Exported allow-list.');
              }}
            >
              Export
            </button>
            <label className="secondary allow-list-import">
              Import
              <input
                type="file"
                accept="application/json,.json"
                hidden
                onChange={(e) => {
                  const file = e.target.files?.[0];
                  e.target.value = '';
                  if (!file) return;
                  void file.text().then((text) => {
                    try {
                      const next = importAllowListJson(text);
                      syncEntries(next);
                      setStatus(`Imported ${next.length} entries.`);
                    } catch (err) {
                      setStatus(err instanceof Error ? err.message : 'Import failed');
                    }
                  });
                }}
              />
            </label>
          </div>
        </div>

        {scanning && progress ? <progress value={progress.scanned} max={progress.total} /> : null}

        {entries.length === 0 ? (
          <p className="hint">Scan installed software or add an executable below.</p>
        ) : (
          <ul className="catalog-list">
            {entries.map((entry) => (
              <li
                key={entry.id}
                className={
                  [
                    entry.present === false ? 'is-inactive' : '',
                    selectedId === entry.id ? 'is-selected' : '',
                  ]
                    .filter(Boolean)
                    .join(' ') || undefined
                }
              >
                <button
                  type="button"
                  className="catalog-row"
                  onClick={() => setSelectedId(entry.id)}
                >
                  <span className="catalog-art">
                    {entry.thumbnailUrl || entry.logoUrl ? (
                      <img src={entry.thumbnailUrl ?? entry.logoUrl ?? ''} alt="" />
                    ) : (
                      <IconFallback name={entry.name} size={24} />
                    )}
                  </span>
                  <span className="catalog-info">
                    <span className="catalog-name">{entry.name}</span>
                    <span className="catalog-meta">
                      <span className="catalog-tag">{entryCategory(entry)}</span>
                      {entry.launchVia ? (
                        <span className="catalog-tag">via {launchViaLabel(entry)}</span>
                      ) : null}
                      {entry.present === false ? (
                        <span className="catalog-warn">Missing</span>
                      ) : null}
                    </span>
                  </span>
                </button>
                <span className="catalog-actions">
                  <button type="button" className="danger" onClick={() => remove(entry.id)}>
                    Remove
                  </button>
                </span>
              </li>
            ))}
          </ul>
        )}

        {manualCandidates.length > 0 ? (
          <div className="allow-list-section">
            <h3 className="allow-list-subhead">Manual import ({manualCandidates.length})</h3>
            <ul className="allow-list allow-list--compact">
              {manualCandidates.map((c) => (
                <li key={c.executablePath} className={c.present ? undefined : 'missing'}>
                  <IconFallback name={c.name} size={22} className="allow-list-icon" />
                  <span className="allow-list-name">
                    {c.name}
                    {c.launchVia ? (
                      <span className="allow-list-category"> via launcher</span>
                    ) : null}
                  </span>
                  <span className="allow-list-actions">
                    <button
                      type="button"
                      className="secondary"
                      disabled={alreadyAdded(c.executablePath)}
                      onClick={() => addCandidate(c)}
                    >
                      {alreadyAdded(c.executablePath) ? 'Added' : 'Add'}
                    </button>
                  </span>
                </li>
              ))}
            </ul>
          </div>
        ) : null}

        <div className="allow-list-section">
          <h3 className="allow-list-subhead">Add manually</h3>
          <form onSubmit={addManual} className="allow-list-manual">
            <input
              value={manualName}
              onChange={(e) => setManualName(e.target.value)}
              placeholder="Display name"
            />
            <input
              value={manualPath}
              onChange={(e) => setManualPath(e.target.value)}
              placeholder="C:\\Path\\To\\app.exe"
            />
            <select
              value={manualCategory}
              aria-label="Section"
              onChange={(e) => setManualCategory(e.target.value as LaunchCategory)}
            >
              {CATEGORY_OPTIONS.map((o) => (
                <option key={o.value} value={o.value}>
                  {o.label}
                </option>
              ))}
            </select>
            <button type="button" className="secondary" onClick={() => void browse()}>
              Browse…
            </button>
            <button type="submit">Add</button>
          </form>
        </div>

        <div className="allow-list-section">
          <h3 className="allow-list-subhead">Game boost</h3>
          <label className="allow-list-boost-toggle">
            <input
              type="checkbox"
              checked={gameBoostEnabled}
              onChange={(e) => {
                const enabled = e.target.checked;
                setGameBoostEnabled(enabled);
                void applyGameBoostSettings({ enabled, aggressive: gameBoostAggressive });
              }}
            />
            Boost performance when launching games
          </label>
          <label className="allow-list-boost-toggle">
            <input
              type="checkbox"
              checked={gameBoostAggressive}
              disabled={!gameBoostEnabled}
              onChange={(e) => {
                const aggressive = e.target.checked;
                setGameBoostAggressive(aggressive);
                void applyGameBoostSettings({ enabled: gameBoostEnabled, aggressive });
              }}
            />
            Close background apps (Discord, browsers, Spotify)
          </label>
          <p className="hint">
            Boost runs automatically on each game launch and resets when the session ends.
          </p>
        </div>

        {status ? <p className="meta">{status}</p> : null}
      </aside>

      <div className="catalog-editor-detail">
        {selected ? (
          <div className="catalog-form">
            <header className="catalog-form-head">
              <div>
                <h2>{selected.name}</h2>
                <p className="hint">
                  Pick optional media from the CDN gallery for Home and Library.
                </p>
              </div>
              <div className="catalog-form-actions catalog-form-actions--head">
                <button
                  type="button"
                  className="secondary"
                  disabled={
                    launchingId !== null ||
                    (selected.launchVia
                      ? !selected.launchVia.executablePath.trim() ||
                        !(normalizeLaunchArguments(selected.launchVia.arguments)?.length ?? 0)
                      : false)
                  }
                  onClick={() => void testLaunch(selected)}
                >
                  {launchingId === selected.id ? 'Launching…' : 'Test launch'}
                </button>
              </div>
            </header>

            <div className="catalog-media">
              <MediaSlot
                label="Logo"
                kind="image"
                url={selected.logoUrl ?? null}
                onPick={() => setPicker({ field: 'logoUrl' })}
                onClear={() => patchSelected({ logoUrl: null })}
              />
              <MediaSlot
                label="Thumbnail"
                kind="image"
                url={selected.thumbnailUrl ?? null}
                onPick={() => setPicker({ field: 'thumbnailUrl' })}
                onClear={() => patchSelected({ thumbnailUrl: null })}
              />
              <MediaSlot
                label="Preview video"
                kind="video"
                url={selected.videoUrl ?? null}
                onPick={() => setPicker({ field: 'videoUrl' })}
                onClear={() => patchSelected({ videoUrl: null })}
              />
            </div>

            <div className="catalog-fields">
              <label className="catalog-field">
                Name
                <input
                  value={selected.name}
                  onChange={(e) => patchSelected({ name: e.target.value })}
                />
              </label>

              <label className="catalog-field">
                {selected.launchVia ? 'Game executable (presence)' : 'Executable path'}
                <input value={selected.executablePath} readOnly />
              </label>

              {entryCategory(selected) === 'game' ? (
                <label className="allow-list-boost-toggle catalog-field--wide">
                  <input
                    type="checkbox"
                    checked={Boolean(selected.launchVia)}
                    onChange={(e) => {
                      if (e.target.checked) {
                        patchSelected({
                          launchVia: {
                            executablePath: '',
                            arguments: '',
                          },
                          arguments: undefined,
                          launchViaAuto: false,
                        });
                      } else {
                        patchSelected({ launchVia: undefined, launchViaAuto: false });
                      }
                    }}
                  />
                  Launch via platform launcher
                </label>
              ) : null}

              {selected.launchVia ? (
                <LaunchViaFields
                  launchVia={selected.launchVia}
                  onBrowseLauncher={() => void browseLauncherExe()}
                  onChange={(launchVia) =>
                    patchSelected({ launchVia, arguments: undefined, launchViaAuto: false })
                  }
                />
              ) : (
                <label className="catalog-field catalog-field--wide">
                  Launch arguments
                  <input
                    value={selected.arguments ?? ''}
                    onChange={(e) => patchSelected({ arguments: e.target.value || undefined })}
                    placeholder="Optional command-line arguments"
                  />
                </label>
              )}

              {selected.launchVia ? (
                <p className="hint">
                  Re-scan updates launcher profiles from trusted sources. Edit fields here to
                  override.
                </p>
              ) : null}

              <label className="catalog-field">
                Section
                <select
                  value={entryCategory(selected)}
                  onChange={(e) => patchSelected({ category: e.target.value as LaunchCategory })}
                >
                  {CATEGORY_OPTIONS.map((o) => (
                    <option key={o.value} value={o.value}>
                      {o.label}
                    </option>
                  ))}
                </select>
              </label>

              {entryCategory(selected) === 'game' ? (
                <label className="catalog-field">
                  Genre
                  <input
                    value={selected.genre ?? ''}
                    onChange={(e) => patchSelected({ genre: e.target.value || null })}
                    placeholder="e.g. Tactical Shooter"
                  />
                </label>
              ) : null}

              <label className="catalog-field catalog-field--wide">
                Description
                <input
                  value={selected.description ?? ''}
                  onChange={(e) => patchSelected({ description: e.target.value || null })}
                  placeholder="Short blurb"
                />
              </label>

              <label className="catalog-field catalog-field--narrow">
                Sort order
                <input
                  {...integerInputProps}
                  value={selected.sortOrder ?? 0}
                  onChange={(e) => patchSelected({ sortOrder: Number(e.target.value) || 0 })}
                />
              </label>
            </div>
          </div>
        ) : (
          <div className="catalog-editor-empty">
            <span className="material-symbols-outlined" aria-hidden="true">
              apps
            </span>
            <p>Select an allowed app to attach gallery media and edit details.</p>
          </div>
        )}
      </div>

      {picker && selected ? (
        <GalleryPicker
          kind={pickerKind}
          value={selected[picker.field] ?? null}
          onSelect={(url) => patchSelected({ [picker.field]: url })}
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

interface LaunchViaFieldsProps {
  launchVia: LaunchVia;
  onBrowseLauncher: () => void;
  onChange: (launchVia: LaunchVia) => void;
}

function LaunchViaFields({ launchVia, onChange, onBrowseLauncher }: LaunchViaFieldsProps) {
  const argsText = formatLaunchArguments(launchVia.arguments);
  const launcherReady = launchVia.executablePath.trim().length > 0;
  return (
    <>
      <label className="catalog-field catalog-field--wide">
        Launcher executable
        <input
          value={launchVia.executablePath}
          onChange={(e) => onChange({ ...launchVia, executablePath: e.target.value })}
          placeholder="C:\\Path\\To\\steam.exe"
        />
        <button type="button" className="secondary" onClick={onBrowseLauncher}>
          Browse…
        </button>
        {!launcherReady ? (
          <span className="hint">Select the platform launcher executable.</span>
        ) : null}
      </label>
      <label className="catalog-field catalog-field--wide">
        Launcher arguments
        <input
          value={argsText}
          onChange={(e) =>
            onChange({
              ...launchVia,
              arguments: tokenizeArguments(e.target.value),
            })
          }
          placeholder="e.g. --launch-product=valorant --launch-patchline=live"
        />
      </label>
    </>
  );
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
            Choose from gallery
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
