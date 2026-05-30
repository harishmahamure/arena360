import { useEffect, useRef, useState } from 'react';

const VOLUME_KEY = 'gaming-cafe.kiosk.volume';

interface KioskTopBarProps {
  playerName: string | null;
  /** Authoritative remaining minutes from the active session, if any. */
  remainingMinutes?: number;
  deviceName?: string | null;
  onEndSession: () => void;
}

function readVolume(): number {
  try {
    const raw = localStorage.getItem(VOLUME_KEY);
    const n = raw === null ? 80 : Number.parseInt(raw, 10);
    return Number.isFinite(n) ? Math.min(100, Math.max(0, n)) : 80;
  } catch {
    return 80;
  }
}

function formatRemaining(minutes?: number): string {
  if (typeof minutes !== 'number') return '--';
  const total = Math.max(0, Math.floor(minutes));
  const h = Math.floor(total / 60);
  const m = total % 60;
  return h > 0 ? `${h}h ${m}m` : `${m}m`;
}

function SpeakerIcon({ muted }: { muted: boolean }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width={22}
      height={22}
      fill="none"
      stroke="currentColor"
      strokeWidth={1.7}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M4 9v6h4l5 4V5L8 9H4z" />
      {muted ? (
        <path d="M16 9l5 5M21 9l-5 5" />
      ) : (
        <path d="M16 8.5a4 4 0 0 1 0 7M18.5 6a7 7 0 0 1 0 12" />
      )}
    </svg>
  );
}

/**
 * Player session top bar (ggCircuit-style): a UI volume control (persisted, no
 * system side effect yet) and a profile dropdown showing the remaining time and
 * an End-session action.
 */
export function KioskTopBar({
  playerName,
  remainingMinutes,
  deviceName,
  onEndSession,
}: KioskTopBarProps) {
  const [open, setOpen] = useState<'audio' | 'profile' | null>(null);
  const [volume, setVolume] = useState<number>(() => readVolume());
  const barRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    try {
      localStorage.setItem(VOLUME_KEY, String(volume));
    } catch {
      // localStorage unavailable — non-fatal
    }
  }, [volume]);

  // Close any open popover on outside click / Escape.
  useEffect(() => {
    if (!open) return;
    const onPointer = (e: PointerEvent) => {
      if (barRef.current && !barRef.current.contains(e.target as Node)) setOpen(null);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') setOpen(null);
    };
    document.addEventListener('pointerdown', onPointer);
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('pointerdown', onPointer);
      document.removeEventListener('keydown', onKey);
    };
  }, [open]);

  const remainingLabel = formatRemaining(remainingMinutes);

  return (
    <div className="kiosk-topbar" ref={barRef}>
      <div className="kiosk-topbar-brand">{deviceName ?? 'Game Zone'}</div>

      <div className="kiosk-topbar-actions">
        <div className="kiosk-audio">
          <button
            type="button"
            className="kiosk-iconbtn"
            aria-label="Volume"
            aria-expanded={open === 'audio'}
            onClick={() => setOpen((p) => (p === 'audio' ? null : 'audio'))}
          >
            <SpeakerIcon muted={volume === 0} />
          </button>
          {open === 'audio' ? (
            <div className="kiosk-popover kiosk-audio-popover" role="dialog" aria-label="Volume">
              <SpeakerIcon muted={volume === 0} />
              <input
                type="range"
                min={0}
                max={100}
                value={volume}
                aria-label="Volume level"
                onChange={(e) => setVolume(Number.parseInt(e.target.value, 10))}
              />
              <span className="kiosk-audio-value">{volume}</span>
            </div>
          ) : null}
        </div>

        <div className="kiosk-profile">
          <button
            type="button"
            className="kiosk-profile-pill"
            aria-expanded={open === 'profile'}
            onClick={() => setOpen((p) => (p === 'profile' ? null : 'profile'))}
          >
            <span className="kiosk-profile-name">{playerName ?? 'Player'}</span>
            <span className="kiosk-profile-time">{remainingLabel}</span>
          </button>
          {open === 'profile' ? (
            <div className="kiosk-popover kiosk-profile-menu" role="menu">
              <div className="kiosk-profile-row">
                <span className="kiosk-profile-label">Time remaining</span>
                <span className="kiosk-profile-strong">{remainingLabel}</span>
              </div>
              {deviceName ? (
                <div className="kiosk-profile-row">
                  <span className="kiosk-profile-label">Station</span>
                  <span>{deviceName}</span>
                </div>
              ) : null}
              <button
                type="button"
                className="danger kiosk-profile-end"
                onClick={() => {
                  setOpen(null);
                  onEndSession();
                }}
              >
                End session
              </button>
            </div>
          ) : null}
        </div>
      </div>
    </div>
  );
}
