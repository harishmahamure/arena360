import { SettingsGlyph, SpeakerGlyph } from '@gaming-cafe/ui/primitives';
import type { CSSProperties } from 'react';
import { useEffect, useRef, useState } from 'react';
import { getSystemVolume, openAudioSettings, setSystemVolume } from '../lib/tauriCommands';

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

function minutesToSeconds(minutes?: number): number | null {
  if (typeof minutes !== 'number') return null;
  return Math.max(0, Math.round(minutes * 60));
}

function formatRemaining(totalSeconds: number | null): string {
  if (totalSeconds === null) return '--';
  const total = Math.max(0, Math.floor(totalSeconds));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}h ${m}m ${s}s`;
  return `${m}m ${s.toString().padStart(2, '0')}s`;
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
  const [remainingSeconds, setRemainingSeconds] = useState<number | null>(() =>
    minutesToSeconds(remainingMinutes),
  );
  const barRef = useRef<HTMLDivElement | null>(null);
  const hasRemainingTime = remainingSeconds !== null;

  useEffect(() => {
    let active = true;
    getSystemVolume()
      .then((systemVolume) => {
        if (active) setVolume(systemVolume);
      })
      .catch(() => {
        // Plain browser/dev fallback keeps the persisted UI value.
      });
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    try {
      localStorage.setItem(VOLUME_KEY, String(volume));
    } catch {
      // localStorage unavailable — non-fatal
    }
  }, [volume]);

  useEffect(() => {
    setRemainingSeconds(minutesToSeconds(remainingMinutes));
  }, [remainingMinutes]);

  useEffect(() => {
    if (!hasRemainingTime) return;
    const id = window.setInterval(() => {
      setRemainingSeconds((current) => {
        if (current === null) return null;
        return Math.max(0, current - 1);
      });
    }, 1000);
    return () => window.clearInterval(id);
  }, [hasRemainingTime]);

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

  const remainingLabel = formatRemaining(remainingSeconds);

  async function changeVolume(next: number) {
    const clamped = Math.min(100, Math.max(0, next));
    setVolume(clamped);
    try {
      const actual = await setSystemVolume(clamped);
      setVolume(actual);
    } catch {
      // Non-Windows/dev fallback remains UI-only.
    }
  }

  return (
    <div className="kiosk-topbar" ref={barRef}>
      <div className="kiosk-topbar-brand">{deviceName ?? 'Game Zone'}</div>

      <div className="kiosk-topbar-actions">
        <div className="kiosk-audio">
          <button
            type="button"
            className={`gz-icon-button kiosk-iconbtn${open === 'audio' ? ' is-active' : ''}`}
            aria-label="Volume"
            aria-expanded={open === 'audio'}
            onClick={() => setOpen((p) => (p === 'audio' ? null : 'audio'))}
          >
            <SpeakerGlyph muted={volume === 0} size={22} />
          </button>
          {open === 'audio' ? (
            <div
              className="gz-popover kiosk-popover kiosk-audio-popover"
              role="dialog"
              aria-label="Volume"
            >
              <SpeakerGlyph muted={volume === 0} size={22} />
              <input
                type="range"
                min={0}
                max={100}
                value={volume}
                style={{ '--kiosk-volume-percent': `${volume}%` } as CSSProperties}
                aria-label="Volume level"
                onChange={(e) => void changeVolume(Number.parseInt(e.target.value, 10))}
              />
              <span className="kiosk-audio-value">{volume}%</span>
              <button
                type="button"
                className="kiosk-audio-settings"
                aria-label="Open Windows sound settings"
                title="Sound settings"
                onClick={() => void openAudioSettings()}
              >
                <SettingsGlyph size={20} />
              </button>
            </div>
          ) : null}
        </div>

        <div className="kiosk-profile">
          <button
            type="button"
            className="gz-profile-pill kiosk-profile-pill"
            aria-expanded={open === 'profile'}
            onClick={() => setOpen((p) => (p === 'profile' ? null : 'profile'))}
          >
            <span className="kiosk-profile-name">{playerName ?? 'Player'}</span>
            <span className="kiosk-profile-time">{remainingLabel}</span>
          </button>
          {open === 'profile' ? (
            <div className="gz-popover kiosk-popover kiosk-profile-menu" role="menu">
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
