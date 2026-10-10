import { FormButton, FormTextField } from '@gaming-cafe/ui';
import { type FormEvent, useEffect, useRef, useState } from 'react';
import type { PortalClient } from '../api';

export const businessAdminUrl = () =>
  import.meta.env.VITE_STAFF_ADMIN_URL ??
  (['localhost', '127.0.0.1', '[::1]'].includes(window.location.hostname)
    ? 'http://localhost:5173/'
    : 'https://staff.arena360.cloud/');

function slugFromName(value: string) {
  return value
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 63)
    .replace(/-+$/g, '');
}

export default function CreateTenantDialog({
  api,
  workerName,
  ready,
  onCreated,
  onClose,
  onBusyChange,
}: {
  api: PortalClient;
  workerName: string;
  ready: boolean;
  onCreated: (id: string) => void;
  onClose: () => void;
  onBusyChange: (busy: boolean) => void;
}) {
  const [step, setStep] = useState<0 | 1 | 2>(0);
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');
  const [slugEdited, setSlugEdited] = useState(false);
  const [timezone, setTimezone] = useState('Asia/Kolkata');
  const [trialDays, setTrialDays] = useState('30');
  const [tenantId, setTenantId] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [showPassword, setShowPassword] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  const submitting = useRef(false);
  const firstField = useRef<HTMLInputElement>(null);
  const completion = useRef<HTMLHeadingElement>(null);
  useEffect(() => {
    if (step === 2) completion.current?.focus();
    else firstField.current?.focus();
  }, [step]);
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (submitting.current) return;
    setError('');
    if (step === 0) {
      if (!ready) {
        setError('Complete the setup checklist before creating a tenant.');
        return;
      }
      if (!name.trim() || new TextEncoder().encode(name.trim()).length > 150) {
        setError(
          'Enter a tenant name of no more than 150 characters. Use a shorter name if needed.',
        );
        return;
      }
      if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(slug)) {
        setError('Use lowercase letters, numbers and single hyphens for the tenant address.');
        return;
      }
      try {
        new Intl.DateTimeFormat('en', { timeZone: timezone });
      } catch {
        setError('Choose a valid timezone, such as Asia/Kolkata or UTC.');
        return;
      }
      if (
        !Number.isInteger(Number(trialDays)) ||
        Number(trialDays) < 1 ||
        Number(trialDays) > 365
      ) {
        setError('Choose a trial between 1 and 365 days.');
        return;
      }
    } else {
      if (password !== confirmation) {
        setError('Passwords do not match.');
        return;
      }
      const size = new TextEncoder().encode(password).length;
      if (size < 12 || size > 72) {
        setError(
          'Use a password of at least 12 characters. Shorten it if it contains many accented characters.',
        );
        return;
      }
    }
    submitting.current = true;
    setBusy(true);
    onBusyChange(true);
    try {
      if (step === 0) {
        const tenant = await api.request<{ id: string }>('/tenants', 'POST', {
          name: name.trim(),
          slug,
          timezone,
          trialDays: Number(trialDays),
        });
        setTenantId(tenant.id);
        setUsername(`${slug}.admin`);
        onCreated(tenant.id);
        setStep(1);
      } else {
        // Retry this operation only: the workspace already exists.
        await api.request(`/tenants/${tenantId}/admins`, 'POST', {
          username: username.trim(),
          password,
        });
        setPassword('');
        setConfirmation('');
        onCreated(tenantId);
        setStep(2);
      }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : String(cause));
    } finally {
      submitting.current = false;
      setBusy(false);
      onBusyChange(false);
    }
  }
  return (
    <div className="modal-overlay">
      <section
        className="modal tenant-create-modal"
        role="dialog"
        aria-modal="true"
        aria-labelledby="tenant-setup-title"
        aria-busy={busy}
      >
        <div className="modal-title">
          <h2 id="tenant-setup-title">
            {step === 0
              ? 'Create tenant'
              : step === 1
                ? 'Create tenant administrator'
                : 'Tenant setup complete'}
          </h2>
          <button type="button" disabled={busy} onClick={onClose} aria-label="Close tenant setup">
            ×
          </button>
        </div>
        <ol className="setup-steps" aria-label="Tenant setup progress">
          {['Workspace', 'Administrator', 'Ready'].map((label, index) => (
            <li
              key={label}
              aria-current={step === index ? 'step' : undefined}
              className={step > index ? 'complete' : ''}
            >
              <span>{step > index ? '✓' : index + 1}</span>
              {label}
            </li>
          ))}
        </ol>
        {step === 2 ? (
          <>
            <h3 ref={completion} tabIndex={-1}>
              Your tenant is ready
            </h3>
            <p>
              <strong>{name.trim()}</strong> is ready to use. Sign in to the staff panel with{' '}
              <strong>{username.trim()}</strong> and the administrator password you just set.
            </p>
            <dl className="setup-summary">
              <dt>Tenant address</dt>
              <dd>{slug}</dd>
              <dt>Timezone</dt>
              <dd>{timezone}</dd>
              <dt>Trial</dt>
              <dd>{trialDays} days</dd>
            </dl>
            <p>Next, add your venues, devices, staff and products in the staff panel.</p>
            <div className="modal-footer">
              <button type="button" onClick={onClose}>
                View tenant details
              </button>
              <a
                className="primary-link"
                href={businessAdminUrl()}
                target="_blank"
                rel="noreferrer"
              >
                Open business admin ↗
              </a>
            </div>
          </>
        ) : (
          <form onSubmit={submit}>
            {step === 0 ? (
              <>
                <p className="tenant-create-lead">
                  Create the customer workspace first. Then set up the account they will use in the
                  staff panel.
                </p>
                <div className="tenant-create-context">
                  <div>
                    <span>DATABASE WORKER</span>
                    <strong>{workerName}</strong>
                  </div>
                  <div>
                    <span>STARTING PLAN</span>
                    <strong>Trial</strong>
                  </div>
                </div>
                <div className="tenant-form-fields">
                  <FormTextField
                    label="Tenant name"
                    name="name"
                    required
                    inputRef={firstField}
                    inputProps={{ maxLength: 150 }}
                    placeholder="Northside Gaming"
                    value={name}
                    disabled={busy}
                    onChange={(event) => {
                      setName(event.target.value);
                      if (!slugEdited) setSlug(slugFromName(event.target.value));
                    }}
                  />
                  <FormTextField
                    label="Slug"
                    name="slug"
                    required
                    value={slug}
                    disabled={busy}
                    inputProps={{ maxLength: 63, pattern: '[a-z0-9]+(-[a-z0-9]+)*' }}
                    helperText="The unique tenant address. Generated from the name; you can edit it."
                    onChange={(event) => {
                      setSlugEdited(true);
                      setSlug(event.target.value.toLowerCase());
                    }}
                  />
                  <div className="tenant-form-row">
                    <FormTextField
                      label="Timezone"
                      name="timezone"
                      required
                      value={timezone}
                      disabled={busy}
                      inputProps={{ list: 'tenant-timezones' }}
                      helperText="Used for daily reports and business hours."
                      onChange={(event) => setTimezone(event.target.value)}
                    />
                    <datalist id="tenant-timezones">
                      <option value="Asia/Kolkata" />
                      <option value="UTC" />
                      <option value="Europe/London" />
                      <option value="America/New_York" />
                    </datalist>
                    <FormTextField
                      label="Trial days"
                      name="trialDays"
                      required
                      type="number"
                      value={trialDays}
                      disabled={busy}
                      inputProps={{ min: 1, max: 365, step: 1 }}
                      helperText="Starts when the workspace is created."
                      onChange={(event) => setTrialDays(event.target.value)}
                    />
                  </div>
                </div>
              </>
            ) : (
              <>
                <div className="notice success" role="status">
                  {name.trim()} has been created. Add its first administrator to finish setup.
                </div>
                <p>
                  This account signs in to the staff panel. Your portal operator account manages
                  tenants separately.
                </p>
                <div className="tenant-form-fields">
                  <FormTextField
                    label="Administrator username"
                    name="username"
                    required
                    inputRef={firstField}
                    inputProps={{ maxLength: 100 }}
                    value={username}
                    disabled={busy}
                    helperText="Use a unique username for this customer's administrator."
                    onChange={(event) => setUsername(event.target.value)}
                  />
                  <FormTextField
                    label="Administrator password"
                    name="password"
                    required
                    type={showPassword ? 'text' : 'password'}
                    value={password}
                    disabled={busy}
                    autoComplete="new-password"
                    inputProps={{ minLength: 12, maxLength: 72 }}
                    helperText="At least 12 characters. Share it directly with the administrator."
                    onChange={(event) => setPassword(event.target.value)}
                  />
                  <FormTextField
                    label="Confirm password"
                    name="confirmation"
                    required
                    type={showPassword ? 'text' : 'password'}
                    value={confirmation}
                    disabled={busy}
                    autoComplete="new-password"
                    onChange={(event) => setConfirmation(event.target.value)}
                  />
                  <button
                    type="button"
                    disabled={busy}
                    aria-pressed={showPassword}
                    onClick={() => setShowPassword(!showPassword)}
                  >
                    {showPassword ? 'Hide passwords' : 'Show passwords'}
                  </button>
                </div>
                {error && (
                  <p className="hint">
                    The tenant is already saved. Retry administrator creation here, or finish later
                    from tenant details.
                  </p>
                )}
              </>
            )}
            {error && (
              <div className="notice error" role="alert">
                {error}
              </div>
            )}
            {busy && (
              <p role="status">
                {step === 0
                  ? 'Creating your workspace and preparing its database…'
                  : 'Creating the administrator account…'}
              </p>
            )}
            <div className="modal-footer">
              <button type="button" disabled={busy} onClick={onClose}>
                {step === 0 ? 'Cancel' : 'Finish later'}
              </button>
              <FormButton
                type="submit"
                className="primary"
                variant="contained"
                loading={busy}
                disabled={step === 0 && !ready}
              >
                {step === 0 ? 'Create tenant' : 'Create administrator'}
              </FormButton>
            </div>
          </form>
        )}
      </section>
    </div>
  );
}
