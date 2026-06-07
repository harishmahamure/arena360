import { type DeductionProfile, deductionPeriodLabelForNow } from '@gaming-cafe/contracts';
import { formatRemainingClock } from '@gaming-cafe/utils';
import type { CSSProperties } from 'react';
import { useEffect, useRef, useState } from 'react';
import { KIOSK_LOGO_URL } from '../../lib/config';
import { getSystemVolume, openAudioSettings, setSystemVolume } from '../../lib/tauriCommands';

const VOLUME_KEY = 'gaming-cafe.kiosk.volume';

export type SessionView = 'home' | 'library' | 'settings';

interface SessionNavProps {
  playerName: string | null;
  deviceName?: string | null;
  /** Authoritative remaining minutes from the active session, if any. */
  remainingMinutes?: number;
  deductionProfile?: DeductionProfile | null;
  cafeTimezone?: string;
  activeView: SessionView;
  onNavigate: (view: SessionView) => void;
  onEndSession: () => void;
  onRefreshTime?: () => Promise<void>;
  refreshing?: boolean;
  onError?: (message: string) => void;
}

const TABS: { id: SessionView; label: string }[] = [
  { id: 'home', label: 'Home' },
  { id: 'library', label: 'Games' },
  { id: 'settings', label: 'Settings' },
];

function readVolume(): number {
  try {
    const raw = localStorage.getItem(VOLUME_KEY);
    const n = raw === null ? 80 : Number.parseInt(raw, 10);
    return Number.isFinite(n) ? Math.min(100, Math.max(0, n)) : 80;
  } catch {
    return 80;
  }
}

/**
 * Arena360 in-session top navigation: brand, Home/Games/Settings tabs, the
 * remaining-time pill, a UI volume control and a profile menu with End session.
 * Volume + countdown logic preserved from the prior KioskTopBar.
 */
export function SessionNav({
  playerName,
  deviceName,
  remainingMinutes,
  deductionProfile,
  cafeTimezone,
  activeView,
  onNavigate,
  onEndSession,
  onRefreshTime,
  refreshing = false,
  onError,
}: SessionNavProps) {
  const [open, setOpen] = useState<'audio' | 'profile' | null>(null);
  const [volume, setVolume] = useState<number>(() => readVolume());
  const barRef = useRef<HTMLDivElement | null>(null);

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

  const remainingLabel = formatRemainingClock(remainingMinutes);
  const deductionPeriod =
    deductionProfile && cafeTimezone
      ? deductionPeriodLabelForNow(deductionProfile, cafeTimezone)
      : null;
  const deductionHint =
    deductionPeriod === 'peak' ? 'Peak pricing' : deductionPeriod === 'low' ? 'Off-peak' : null;

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

  async function handleOpenAudioSettings() {
    try {
      await openAudioSettings();
      setOpen(null);
    } catch (error) {
      onError?.(error instanceof Error ? error.message : 'Could not open sound settings');
    }
  }

  return (
    <nav className="a360-nav" ref={barRef}>
      <div className="a360-nav-left">
        {KIOSK_LOGO_URL ? (
          <img className="a360-nav-logo" src={KIOSK_LOGO_URL} alt="Arena360" />
        ) : (
          <span className="a360-nav-brand">ARENA360</span>
        )}
        <div className="a360-nav-tabs">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              type="button"
              className={`a360-tab${activeView === tab.id ? ' is-active' : ''}`}
              aria-current={activeView === tab.id ? 'page' : undefined}
              onClick={() => onNavigate(tab.id)}
            >
              {tab.label}
            </button>
          ))}
        </div>
      </div>

      <div className="a360-nav-right">
        <div className="a360-time-group">
          <div className="a360-time-pill" title={deductionHint ?? undefined}>
            <span className="material-symbols-outlined is-filled">timer</span>
            <span>{remainingLabel}</span>
            {deductionHint ? <span className="a360-time-hint">{deductionHint}</span> : null}
          </div>
          {onRefreshTime ? (
            <button
              type="button"
              className="kiosk-iconbtn a360-time-refresh"
              aria-label="Refresh remaining time"
              disabled={refreshing}
              onClick={() => void onRefreshTime()}
            >
              <span className={`material-symbols-outlined${refreshing ? ' is-spinning' : ''}`}>
                refresh
              </span>
            </button>
          ) : null}
        </div>

        <div className="kiosk-audio">
          <button
            type="button"
            className={`kiosk-iconbtn${open === 'audio' ? ' is-active' : ''}`}
            aria-label="Volume"
            aria-expanded={open === 'audio'}
            onClick={() => setOpen((p) => (p === 'audio' ? null : 'audio'))}
          >
            <span className="material-symbols-outlined">
              {volume === 0 ? 'volume_off' : 'volume_up'}
            </span>
          </button>
          {open === 'audio' ? (
            <div
              className="gz-popover kiosk-popover kiosk-audio-popover"
              role="dialog"
              aria-label="Volume"
            >
              <span className="material-symbols-outlined">
                {volume === 0 ? 'volume_off' : 'volume_up'}
              </span>
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
                onClick={() => void handleOpenAudioSettings()}
              >
                <span className="material-symbols-outlined">settings</span>
              </button>
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
    </nav>
  );
}
