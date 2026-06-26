import {
  ApiError,
  normalizeUsername,
  sanitizeUsernameInput,
  trimValue,
  USERNAME_HELPER_TEXT,
} from '@gaming-cafe/utils';
import { useEffect, useRef, useState } from 'react';
import { ValidationError } from 'yup';
import { StationControls } from '../components/StationControls';
import { useKiosk } from '../context/KioskProvider';
import {
  BUNDLED_LOGIN_BACKGROUND_VIDEO,
  KIOSK_LOGO_URL,
  LOGIN_BACKGROUND_VIDEO_URL,
} from '../lib/config';
import { playerLoginPasswordInputProps, playerLoginUsernameInputProps } from '../lib/inputHints';
import { kioskPlayerRegisterSchema } from '../lib/kioskPlayerRegisterSchema';
import {
  firstRegistrationFieldWithError,
  mapRegistrationApiError,
  type RegistrationField,
  type RegistrationFormErrors,
} from '../lib/registrationErrors';
import { cachedAssetSrc } from '../lib/tauriCommands';

const FIELD_IDS: Record<RegistrationField, string> = {
  username: 'reg-username',
  phoneNumber: 'reg-phone',
  password: 'reg-password',
  confirmPassword: 'reg-confirm-password',
  firstName: 'reg-first-name',
  lastName: 'reg-last-name',
};

function yupErrorsToFieldErrors(error: ValidationError): RegistrationFormErrors {
  const next: RegistrationFormErrors = {};
  for (const issue of error.inner.length > 0 ? error.inner : [error]) {
    if (!issue.path) continue;
    const field = issue.path as RegistrationField;
    if (!next[field]) next[field] = issue.message;
  }
  return next;
}

export function CreateAccountPage() {
  const { registerPlayer, backToLoginFromCreateAccount, online, maintenance, deviceName } =
    useKiosk();

  const [username, setUsername] = useState('');
  const [phoneNumber, setPhoneNumber] = useState('');
  const [password, setPassword] = useState('');
  const [confirmPassword, setConfirmPassword] = useState('');
  const [firstName, setFirstName] = useState('');
  const [lastName, setLastName] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [showConfirmPassword, setShowConfirmPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [fieldErrors, setFieldErrors] = useState<RegistrationFormErrors>({});
  const [videoSrc, setVideoSrc] = useState<string | null>(null);
  const videoFallbackIndex = useRef(0);
  const videoFallbacks = useRef<string[]>([]);

  const blocked = maintenance || !online;

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

  function clearFieldError(field: RegistrationField) {
    setFieldErrors((prev) => {
      if (!prev[field]) return prev;
      const next = { ...prev };
      delete next[field];
      return next;
    });
  }

  function focusFirstError(errors: RegistrationFormErrors) {
    const field = firstRegistrationFieldWithError(errors);
    if (!field) return;
    document.getElementById(FIELD_IDS[field])?.focus();
  }

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

  async function onSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (blocked || busy) return;

    setFieldErrors({});

    const payload = {
      username: normalizeUsername(username),
      phoneNumber: trimValue(phoneNumber),
      password: trimValue(password),
      confirmPassword: trimValue(confirmPassword),
      firstName: trimValue(firstName) || undefined,
      lastName: trimValue(lastName) || undefined,
    };

    try {
      await kioskPlayerRegisterSchema.validate(payload, { abortEarly: false });
    } catch (err) {
      if (err instanceof ValidationError) {
        const errors = yupErrorsToFieldErrors(err);
        setFieldErrors(errors);
        focusFirstError(errors);
      }
      return;
    }

    setBusy(true);
    try {
      await registerPlayer({
        username: payload.username,
        phoneNumber: payload.phoneNumber,
        password: payload.password,
        firstName: payload.firstName,
        lastName: payload.lastName,
      });
    } catch (err) {
      if (err instanceof ApiError) {
        const errors = mapRegistrationApiError(err);
        setFieldErrors(errors);
        focusFirstError(errors);
      } else {
        setFieldErrors({ form: 'Registration failed. Please try again.' });
      }
    } finally {
      setBusy(false);
    }
  }

  function renderFieldError(field: RegistrationField) {
    const message = fieldErrors[field];
    if (!message) return null;
    const id = `${FIELD_IDS[field]}-error`;
    return (
      <p className="a360-field-error" id={id} role="alert">
        {message}
      </p>
    );
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
            onError={onVideoError}
          />
        </div>
      ) : null}
      <div className="a360-radial-overlay" />

      <main className="a360-login-card a360-login-card--wide">
        <header className="a360-login-header">
          {KIOSK_LOGO_URL ? (
            <img className="a360-login-logo" src={KIOSK_LOGO_URL} alt="" />
          ) : (
            <span className="a360-brand">ARENA360</span>
          )}
          <h1 className="a360-login-title">Create your account</h1>
          <p className="a360-login-subtitle">Register to play at this station</p>
        </header>

        {maintenance ? (
          <div className="maintenance-banner" role="alert">
            <p className="error-headline">Station under maintenance</p>
            <p className="error-detail">Registration is unavailable. Please ask staff for help.</p>
          </div>
        ) : !online ? (
          <div className="maintenance-banner" role="alert">
            <p className="error-headline">Reconnecting…</p>
            <p className="error-detail">
              Registration is unavailable until the connection returns.
            </p>
          </div>
        ) : null}

        <form className="a360-form" autoComplete="off" onSubmit={onSubmit} noValidate>
          <div className="a360-field">
            <label className="a360-field-label" htmlFor={FIELD_IDS.username}>
              Username
            </label>
            <div className="a360-input-wrap">
              <input
                id={FIELD_IDS.username}
                value={username}
                onChange={(e) => {
                  setUsername(sanitizeUsernameInput(e.target.value));
                  clearFieldError('username');
                }}
                placeholder="Choose a username"
                disabled={blocked}
                aria-invalid={Boolean(fieldErrors.username)}
                aria-describedby={
                  fieldErrors.username ? `${FIELD_IDS.username}-error` : 'reg-username-hint'
                }
                {...playerLoginUsernameInputProps}
                required
              />
            </div>
            <p className="a360-field-hint" id="reg-username-hint">
              {USERNAME_HELPER_TEXT}
            </p>
            {renderFieldError('username')}
          </div>

          <div className="a360-field">
            <label className="a360-field-label" htmlFor={FIELD_IDS.phoneNumber}>
              Phone number
            </label>
            <div className="a360-input-wrap">
              <input
                id={FIELD_IDS.phoneNumber}
                value={phoneNumber}
                onChange={(e) => {
                  setPhoneNumber(e.target.value.replace(/\D/g, '').slice(0, 15));
                  clearFieldError('phoneNumber');
                }}
                placeholder="9876543210"
                disabled={blocked}
                inputMode="numeric"
                autoComplete="off"
                aria-invalid={Boolean(fieldErrors.phoneNumber)}
                aria-describedby={
                  fieldErrors.phoneNumber ? `${FIELD_IDS.phoneNumber}-error` : undefined
                }
                required
              />
            </div>
            {renderFieldError('phoneNumber')}
          </div>

          <div className="a360-form-row">
            <div className="a360-field">
              <label className="a360-field-label" htmlFor={FIELD_IDS.firstName}>
                First name (optional)
              </label>
              <div className="a360-input-wrap">
                <input
                  id={FIELD_IDS.firstName}
                  value={firstName}
                  onChange={(e) => {
                    setFirstName(e.target.value);
                    clearFieldError('firstName');
                  }}
                  disabled={blocked}
                  autoComplete="off"
                  maxLength={50}
                  aria-invalid={Boolean(fieldErrors.firstName)}
                  aria-describedby={
                    fieldErrors.firstName ? `${FIELD_IDS.firstName}-error` : undefined
                  }
                />
              </div>
              {renderFieldError('firstName')}
            </div>

            <div className="a360-field">
              <label className="a360-field-label" htmlFor={FIELD_IDS.lastName}>
                Last name (optional)
              </label>
              <div className="a360-input-wrap">
                <input
                  id={FIELD_IDS.lastName}
                  value={lastName}
                  onChange={(e) => {
                    setLastName(e.target.value);
                    clearFieldError('lastName');
                  }}
                  disabled={blocked}
                  autoComplete="off"
                  maxLength={50}
                  aria-invalid={Boolean(fieldErrors.lastName)}
                  aria-describedby={
                    fieldErrors.lastName ? `${FIELD_IDS.lastName}-error` : undefined
                  }
                />
              </div>
              {renderFieldError('lastName')}
            </div>
          </div>

          <div className="a360-field">
            <label className="a360-field-label" htmlFor={FIELD_IDS.password}>
              Password
            </label>
            <div className="a360-input-wrap">
              <input
                id={FIELD_IDS.password}
                type={showPassword ? 'text' : 'password'}
                value={password}
                onChange={(e) => {
                  setPassword(e.target.value);
                  clearFieldError('password');
                }}
                placeholder="At least 8 characters"
                disabled={blocked}
                aria-invalid={Boolean(fieldErrors.password)}
                aria-describedby={fieldErrors.password ? `${FIELD_IDS.password}-error` : undefined}
                {...playerLoginPasswordInputProps}
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
            {renderFieldError('password')}
          </div>

          <div className="a360-field">
            <label className="a360-field-label" htmlFor={FIELD_IDS.confirmPassword}>
              Confirm password
            </label>
            <div className="a360-input-wrap">
              <input
                id={FIELD_IDS.confirmPassword}
                type={showConfirmPassword ? 'text' : 'password'}
                value={confirmPassword}
                onChange={(e) => {
                  setConfirmPassword(e.target.value);
                  clearFieldError('confirmPassword');
                }}
                placeholder="Repeat your password"
                disabled={blocked}
                aria-invalid={Boolean(fieldErrors.confirmPassword)}
                aria-describedby={
                  fieldErrors.confirmPassword ? `${FIELD_IDS.confirmPassword}-error` : undefined
                }
                {...playerLoginPasswordInputProps}
                required
              />
              <button
                type="button"
                className="a360-eye"
                aria-label={showConfirmPassword ? 'Hide password' : 'Show password'}
                aria-pressed={showConfirmPassword}
                onClick={() => setShowConfirmPassword((v) => !v)}
              >
                <span className="material-symbols-outlined">
                  {showConfirmPassword ? 'visibility_off' : 'visibility'}
                </span>
              </button>
            </div>
            {renderFieldError('confirmPassword')}
          </div>

          {fieldErrors.form ? (
            <div className="gz-auth-error" role="alert">
              <p className="error-headline">{fieldErrors.form}</p>
            </div>
          ) : null}

          <button type="submit" className="primary-glow-btn" disabled={blocked || busy}>
            {busy ? 'Creating account…' : 'Create account'}
          </button>
        </form>

        <StationControls deviceName={deviceName} online={online} maintenance={maintenance} />

        <footer className="a360-login-footer">
          <button
            type="button"
            className="link a360-staff-login"
            onClick={backToLoginFromCreateAccount}
          >
            Back to sign in
          </button>
        </footer>
      </main>
    </div>
  );
}
