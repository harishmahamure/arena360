import { useEffect, useRef, useState } from 'react';
import { StationControls } from '../components/StationControls';
import { useKiosk } from '../context/KioskProvider';
import { SESSION_EXPIRED_DISMISS_MS, SESSION_EXPIRED_MESSAGE } from '../lib/authMessages';
import {
  BUNDLED_LOGIN_BACKGROUND_VIDEO,
  KIOSK_APP_VERSION,
  KIOSK_LOGO_URL,
  LOGIN_BACKGROUND_VIDEO_URL,
} from '../lib/config';
import {
  clearFailures,
  getLockout,
  recordFailure,
  resetLoginLockoutByStaff,
} from '../lib/loginLockout';
import { cachedAssetSrc } from '../lib/tauriCommands';

function formatRetry(retryAt: number): string {
  const mins = Math.max(1, Math.ceil((retryAt - Date.now()) / 60000));
  return `${mins} minute${mins === 1 ? '' : 's'}`;
}

/**
 * Arena360 cinematic logged-out home: looped background video with radial scrim,
 * centered glass sign-in card, and station controls. All auth logic (lockout,
 * maintenance/offline gating, player login) is preserved. Setup is reached via
 * Ctrl+Shift+A (handled globally in KioskProvider). Staff can clear login lockout
 * with Ctrl+Shift+B on this screen.
 */
export function LoginHomePage() {
  const {
    playerLogin,
    error,
    clearError,
    online,
    maintenance,
    deviceName,
    loginNotice,
    clearLoginNotice,
  } = useKiosk();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [lockout, setLockout] = useState(() => getLockout());
  const [staffLockCleared, setStaffLockCleared] = useState(false);
  const [videoSrc, setVideoSrc] = useState<string | null>(null);
  const videoFallbackIndex = useRef(0);

  const videoFallbacks = useRef<string[]>([]);

  useEffect(() => {
    if (!lockout.locked) return;
    const id = setInterval(() => setLockout(getLockout()), 10000);
    return () => clearInterval(id);
  }, [lockout.locked]);

  useEffect(() => {
    if (error !== SESSION_EXPIRED_MESSAGE || lockout.locked) return;
    const id = setTimeout(() => clearError(), SESSION_EXPIRED_DISMISS_MS);
    return () => clearTimeout(id);
  }, [error, lockout.locked, clearError]);

  // Staff-only: clear client login lockout (same hidden shortcut pattern as Ctrl+Shift+A setup).
  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if (e.ctrlKey && e.shiftKey && (e.key === 'B' || e.key === 'b')) {
        e.preventDefault();
        setLockout(resetLoginLockoutByStaff());
        setStaffLockCleared(true);
      }
    }
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      const cached = await cachedAssetSrc(LOGIN_BACKGROUND_VIDEO_URL);
      const sources = [cached, BUNDLED_LOGIN_BACKGROUND_VIDEO].filter(
        (src, index, all) => src && all.indexOf(src) === index,
      );
      videoFallbacks.current = sources;
      videoFallbackIndex.current = 0;
      if (!cancelled && sources[0]) {
        setVideoSrc(sources[0]);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  function onVideoError() {
    const sources = videoFallbacks.current;
    const next = videoFallbackIndex.current + 1;
    if (next < sources.length) {
      videoFallbackIndex.current = next;
      setVideoSrc(sources[next] ?? null);
      return;
    }
    setVideoSrc(null);
  }

  const blocked = maintenance || !online || lockout.locked;

  function tryAutoplay(video: HTMLVideoElement) {
    void Promise.resolve(video.play()).catch(() => {
      // Autoplay may be blocked until user interaction; muted + playsInline usually succeeds.
    });
  }

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (blocked) return;
    setBusy(true);
    try {
      await playerLogin(username, password);
      clearFailures();
    } catch {
      setLockout(recordFailure());
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="a360-login login-home">
      {videoSrc ? (
        <div className="a360-login-media" aria-hidden="true">
          <video
            src={videoSrc}
            autoPlay
            loop
            muted
            playsInline
            preload="auto"
            onLoadedData={(e) => tryAutoplay(e.currentTarget)}
            onError={onVideoError}
          />
        </div>
      ) : null}
      <div className="a360-radial-overlay" />

      <main className="a360-login-card">
        <header className="a360-login-header">
          {KIOSK_LOGO_URL ? (
            <img className="a360-login-logo" src={KIOSK_LOGO_URL} alt="" />
          ) : (
            <span className="a360-brand">ARENA360</span>
          )}
          <h1 className="a360-login-title">Welcome back</h1>
          <p className="a360-login-subtitle">Enter your credentials to access your terminal</p>
        </header>

        {loginNotice ? (
          <div className="maintenance-banner" role="status">
            <p className="error-headline">{loginNotice}</p>
            <p className="error-detail">You can sign in again when you are ready.</p>
            <button type="button" className="secondary" onClick={clearLoginNotice}>
              Dismiss
            </button>
          </div>
        ) : null}

        {maintenance ? (
          <div className="maintenance-banner" role="alert">
            <p className="error-headline">Station under maintenance</p>
            <p className="error-detail">Please ask staff for assistance.</p>
          </div>
        ) : !online ? (
          <div className="maintenance-banner" role="alert">
            <p className="error-headline">Reconnecting…</p>
            <p className="error-detail">Sign-in is unavailable until the connection returns.</p>
          </div>
        ) : null}

        <form className="a360-form" onSubmit={onSubmit}>
          <div className="a360-field">
            <label className="a360-field-label" htmlFor="kiosk-username">
              Username
            </label>
            <div className="a360-input-wrap">
              <input
                id="kiosk-username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Enter your username"
                disabled={blocked}
                autoComplete="off"
                required
              />
            </div>
          </div>

          <div className="a360-field">
            <label className="a360-field-label" htmlFor="kiosk-password">
              Password
            </label>
            <div className="a360-input-wrap">
              <input
                id="kiosk-password"
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="••••••••"
                disabled={blocked}
                required
              />
              <button
                type="button"
                className="a360-eye"
                aria-label={showPassword ? 'Hide password' : 'Show password'}
                aria-pressed={showPassword}
                onClick={() => setShowPassword((v) => !v)}
              >
                <span className="material-symbols-outlined">
                  {showPassword ? 'visibility_off' : 'visibility'}
                </span>
              </button>
            </div>
          </div>

          {staffLockCleared && !lockout.locked ? (
            <div className="maintenance-banner" role="status">
              <p className="error-headline">Sign-in lock cleared</p>
              <p className="error-detail">The player can try again now.</p>
            </div>
          ) : lockout.locked ? (
            <div className="gz-auth-error" role="alert">
              <p className="error-headline">Too many attempts</p>
              <p className="error-detail">
                Sign-in is locked. Try again in {formatRetry(lockout.retryAt)} or ask staff for
                help.
              </p>
            </div>
          ) : error ? (
            <div className="gz-auth-error" role="alert">
              {error.split('\n').map((line) => (
                <p
                  key={line}
                  className={line.startsWith('No usable plan') ? 'error-headline' : 'error-detail'}
                >
                  {line}
                </p>
              ))}
            </div>
          ) : null}

          <button type="submit" className="primary-glow-btn" disabled={blocked || busy}>
            {busy ? 'Signing in…' : 'Sign in'}
          </button>
        </form>

        <StationControls deviceName={deviceName} online={online} maintenance={maintenance} />

        {/* biome-ignore lint/a11y/useAriaPropsSupportedByRole: version line exposes build label to screen readers */}
        <p className="a360-login-version" aria-label={`App version ${KIOSK_APP_VERSION}`}>
          v{KIOSK_APP_VERSION}
        </p>
      </main>
    </div>
  );
}
