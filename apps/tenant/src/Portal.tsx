import { EmptyState, MetricCard, StatusBadge } from '@gaming-cafe/ui';
import { type FormEvent, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import QRCode from 'react-qr-code';
import { type Cell, type Detail, type Overview, type Plan, PortalClient, type Tenant } from './api';
import CreateTenantDialog, { businessAdminUrl } from './components/CreateTenantDialog';
import SetupChecklist from './components/SetupChecklist';

const date = (value: string | null) => (value ? new Date(value).toLocaleString() : '—');
const localDateTime = (value: string | null) => {
  if (!value) return '';
  const d = new Date(value);
  return new Date(d.getTime() - d.getTimezoneOffset() * 60000).toISOString().slice(0, 16);
};
const short = (value: string | null) => (value ? value.slice(0, 8) : 'Unassigned');
function Status({ value }: { value: string }) {
  const tone = ['ACTIVE', 'READY', 'COMPLETE'].includes(value)
    ? 'success'
    : ['FAILED', 'FENCED', 'OFFLINE', 'DISABLED'].includes(value)
      ? 'error'
      : 'default';
  return <StatusBadge label={value} tone={tone} />;
}
type Modal =
  | 'tenant'
  | 'cell'
  | 'admin'
  | 'timezone'
  | 'move'
  | 'cold'
  | 'wake'
  | 'subscription'
  | 'plan';
export default function Portal() {
  const [token, setToken] = useState('');
  const [challenge, setChallenge] = useState('');
  const [setupRequired, setSetupRequired] = useState(false);
  const [setup, setSetup] = useState<{ secret: string; otpauthUri: string }>();
  const [connecting, setConnecting] = useState(false);
  const [loginError, setLoginError] = useState('');
  async function connect(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    setConnecting(true);
    setLoginError('');
    try {
      const login = await new PortalClient('').auth<{ challenge: string; setupRequired: boolean }>(
        '/login',
        {
          username: String(data.get('username') ?? '').trim(),
          password: String(data.get('password') ?? ''),
        },
      );
      setChallenge(login.challenge);
      setSetupRequired(login.setupRequired);
      if (login.setupRequired) {
        setSetup(
          await new PortalClient(login.challenge).auth<{ secret: string; otpauthUri: string }>(
            '/totp/setup',
          ),
        );
      }
    } catch (error) {
      setLoginError(String(error instanceof Error ? error.message : error));
    } finally {
      setConnecting(false);
    }
  }
  async function verify(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setConnecting(true);
    setLoginError('');
    try {
      const code = String(new FormData(event.currentTarget).get('code') ?? '');
      const session = await new PortalClient(challenge).auth<{ token: string }>('/totp/verify', {
        code,
      });
      setToken(session.token);
      setChallenge('');
      setSetup(undefined);
    } catch (error) {
      setLoginError(String(error instanceof Error ? error.message : error));
    } finally {
      setConnecting(false);
    }
  }
  async function logout() {
    try {
      await new PortalClient(token).auth('/logout');
    } catch {
      /* The local session still ends. */
    }
    setToken('');
  }
  if (!token)
    return (
      <div className="login">
        <div className="login-brand">
          <div className="brand-mark">A</div>
          <span>
            ARENA<span className="brand-360">360</span>
          </span>
        </div>
        <div className="login-card">
          <span className="eyebrow">PLATFORM OPERATIONS</span>
          <h1>
            Your tenants.
            <br />
            One control plane.
          </h1>
          <p>Manage tenant lifecycle, cell placement and background work from one place.</p>
          <form onSubmit={challenge ? verify : connect}>
            {challenge ? (
              <>
                <p>
                  {setupRequired
                    ? 'Set up an authenticator app before entering the portal.'
                    : 'Enter the code from your authenticator app.'}
                </p>
                {setup && (
                  <div className="totp-setup">
                    <QRCode value={setup.otpauthUri} size={160} />
                    <p>
                      Or enter this key manually: <code>{setup.secret}</code>
                    </p>
                  </div>
                )}
                <label htmlFor="operator-code">Authentication code</label>
                <input
                  id="operator-code"
                  name="code"
                  inputMode="numeric"
                  pattern="[0-9]{6}"
                  maxLength={6}
                  autoComplete="one-time-code"
                  required
                />
              </>
            ) : (
              <>
                <label htmlFor="operator-username">Username</label>
                <input id="operator-username" name="username" autoComplete="username" required />
                <label htmlFor="operator-password">Password</label>
                <input
                  id="operator-password"
                  name="password"
                  type="password"
                  autoComplete="current-password"
                  required
                />
              </>
            )}
            {loginError && (
              <div className="notice error" role="alert">
                {loginError}
              </div>
            )}
            <button type="submit" className="primary full" disabled={connecting}>
              {connecting ? 'Working…' : challenge ? 'Verify and continue →' : 'Continue →'}
            </button>
            {challenge && (
              <button
                type="button"
                onClick={() => {
                  setChallenge('');
                  setSetup(undefined);
                  setLoginError('');
                }}
              >
                Back to sign in
              </button>
            )}
          </form>
        </div>
        <span className="login-footer">Tenant management portal · PostgreSQL control plane</span>
      </div>
    );
  return <Console token={token} logout={logout} />;
}
function Console({ token, logout }: { token: string; logout: () => void }) {
  const api = useMemo(() => new PortalClient(token), [token]);
  const [tab, setTab] = useState<'tenants' | 'cells' | 'plans'>('tenants');
  const [overview, setOverview] = useState<Overview>();
  const [tenants, setTenants] = useState<Tenant[]>([]);
  const [cells, setCells] = useState<Cell[]>([]);
  const [plans, setPlans] = useState<Plan[]>([]);
  const [editingPlan, setEditingPlan] = useState<Plan>();
  const [search, setSearch] = useState('');
  const [filter, setFilter] = useState('');
  const [offset, setOffset] = useState(0);
  const [selected, setSelected] = useState<string | null>(null);
  const [detail, setDetail] = useState<Detail>();
  const [error, setError] = useState('');
  const [detailError, setDetailError] = useState('');
  const [message, setMessage] = useState('');
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [modal, setModal] = useState<Modal | null>(null);
  const [modalError, setModalError] = useState('');
  const [revision, setRevision] = useState(0);
  const [updated, setUpdated] = useState('');
  const requestVersion = useRef(0);
  const refresh = useCallback(() => setRevision((v) => v + 1), []);
  useEffect(() => {
    const timer = window.setInterval(refresh, 15000);
    return () => window.clearInterval(timer);
  }, [refresh]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: revision is the explicit refresh trigger.
  useEffect(() => {
    const controller = new AbortController();
    const version = ++requestVersion.current;
    setLoading(true);
    const params = new URLSearchParams({
      search,
      state: filter,
      offset: String(offset),
      limit: '30',
    });
    Promise.all([
      api.request<Overview>('/overview', 'GET', undefined, controller.signal),
      api.request<{ items: Tenant[] }>(`/tenants?${params}`, 'GET', undefined, controller.signal),
      api.request<Cell[]>('/cells', 'GET', undefined, controller.signal),
      api.request<Plan[]>('/plans', 'GET', undefined, controller.signal),
    ])
      .then(([o, t, c, p]) => {
        if (version !== requestVersion.current || controller.signal.aborted) return;
        setOverview(o);
        setTenants(t.items);
        setCells(c);
        setPlans(p);
        setError('');
        setUpdated(new Date().toLocaleTimeString());
      })
      .catch((e) => {
        if (!controller.signal.aborted) setError(e.message);
      })
      .finally(() => {
        if (!controller.signal.aborted) setLoading(false);
      });
    return () => controller.abort();
  }, [api, search, filter, offset, revision]);
  // biome-ignore lint/correctness/useExhaustiveDependencies: revision refreshes selected tenant details.
  useEffect(() => {
    if (!selected) {
      setDetail(undefined);
      return;
    }
    const controller = new AbortController();
    setDetailError('');
    api
      .request<Detail>(`/tenants/${selected}`, 'GET', undefined, controller.signal)
      .then((d) => {
        if (!controller.signal.aborted) setDetail(d);
      })
      .catch((e) => {
        if (!controller.signal.aborted) {
          setDetail(undefined);
          setDetailError(e.message);
        }
      });
    return () => controller.abort();
  }, [api, selected, revision]);
  useEffect(() => {
    if (!modal && !selected) return;
    const previous = document.activeElement as HTMLElement | null;
    const dialogs = document.querySelectorAll<HTMLElement>('[role="dialog"]');
    const dialog = dialogs[dialogs.length - 1];
    const focusable = () =>
      Array.from(
        dialog?.querySelectorAll<HTMLElement>(
          'button:not(:disabled), input, select, a[href], [tabindex="0"]',
        ) ?? [],
      );
    (modal === 'tenant'
      ? dialog?.querySelector<HTMLElement>('input[name="name"]')
      : focusable()[0]
    )?.focus();
    function keyboard(event: KeyboardEvent) {
      if (event.key === 'Escape' && !busy) {
        if (modal) setModal(null);
        else setSelected(null);
      }
      if (event.key !== 'Tab') return;
      const elements = focusable();
      const first = elements[0];
      const last = elements[elements.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    }
    document.addEventListener('keydown', keyboard);
    return () => {
      document.removeEventListener('keydown', keyboard);
      previous?.focus();
    };
  }, [modal, selected, busy]);
  function open(kind: Modal) {
    setModalError('');
    setModal(kind);
  }
  function pick(id: string) {
    setDetail(undefined);
    setSelected(id);
  }
  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const values = Object.fromEntries(data);
    setBusy(true);
    setModalError('');
    const id = selected;
    let path = '';
    let method = 'POST';
    let body: unknown;
    try {
      switch (modal) {
        case 'cell':
          path = '/cells';
          body = { name: values.name, address: values.address, id: values.id || null };
          break;
        case 'admin':
          path = `/tenants/${id}/admins`;
          body = { username: values.username, password: values.password };
          break;
        case 'timezone':
          path = `/tenants/${id}/timezone`;
          method = 'PUT';
          body = { timezone: values.timezone };
          break;
        case 'move':
          path = `/tenants/${id}/move`;
          body = { targetCell: values.targetCell };
          break;
        case 'cold':
          path = `/tenants/${id}/cold`;
          body = { minimumIdleSeconds: Number(values.minimumIdleSeconds) };
          break;
        case 'wake':
          path = `/tenants/${id}/wake`;
          break;
        case 'subscription':
          path = `/tenants/${id}/subscription`;
          method = 'PUT';
          body = {
            planCode: values.planCode,
            endsAt: new Date(String(values.endsAt)).toISOString(),
          };
          break;
        case 'plan':
          path = editingPlan ? `/plans/${editingPlan.code}` : '/plans';
          method = editingPlan ? 'PUT' : 'POST';
          body = {
            ...(editingPlan ? {} : { code: values.code }),
            name: values.name,
            graceDays: Number(values.graceDays),
            entitlements: JSON.parse(String(values.entitlements)),
            isActive: values.isActive === 'on',
          };
          break;
        default:
          setBusy(false);
          return;
      }
      const result = await api.request<{ id?: string; message?: string }>(path, method, body);
      setModal(null);
      setMessage(result.message ?? 'Request completed. Current state and jobs are shown below.');
      refresh();
    } catch (e) {
      setModalError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }
  async function cancelJob(job: Detail['jobs'][number]) {
    if (!selected) return;
    setBusy(true);
    try {
      await api.request(`/tenants/${selected}/jobs/${job.id}/cancel`, 'POST', { kind: job.kind });
      setMessage('Job cancelled.');
      refresh();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }
  async function toggleTenant() {
    if (!selected || !t) return;
    setBusy(true);
    try {
      await api.request(`/tenants/${selected}/enabled`, 'PUT', { enabled: !t.is_enabled });
      setMessage(t.is_enabled ? 'Tenant deactivated.' : 'Tenant activated.');
      refresh();
    } catch (e) {
      setDetailError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }
  const t = detail?.tenant;
  const worker = cells.find((cell) => cell.id === overview?.localCellId);
  const trialReady = plans.some((plan) => plan.code === 'trial' && plan.isActive);
  const provisionReady = Boolean(
    overview?.canProvision && worker?.state === 'ACTIVE' && trialReady && !error,
  );
  const titles: Record<Modal, string> = {
    tenant: 'Create tenant',
    cell: 'Register cell',
    admin: 'Create tenant administrator',
    timezone: 'Change timezone',
    move: 'Move tenant',
    cold: 'Put tenant into cold storage',
    wake: 'Wake tenant',
    subscription: 'Update plan and expiry',
    plan: editingPlan ? 'Edit plan' : 'Create plan',
  };
  return (
    <div className="portal-shell">
      <aside className="sidebar" inert={Boolean(selected || modal)}>
        <div className="wordmark">
          <div className="brand-mark">A</div>ARENA<span>360</span>
        </div>
        <div className="workspace-label">PLATFORM WORKSPACE</div>
        <nav>
          <button
            type="button"
            className={tab === 'tenants' ? 'nav-active' : ''}
            onClick={() => setTab('tenants')}
          >
            <span>▦</span> Tenants <small>{overview?.counts.total ?? '—'}</small>
          </button>
          <button
            type="button"
            className={tab === 'cells' ? 'nav-active' : ''}
            onClick={() => setTab('cells')}
          >
            <span>▧</span> Cells <small>{cells.length}</small>
          </button>
          <button
            type="button"
            className={tab === 'plans' ? 'nav-active' : ''}
            onClick={() => setTab('plans')}
          >
            <span>▤</span> Plans <small>{plans.length}</small>
          </button>
        </nav>
        <div className="sidebar-bottom">
          <div className="connection">
            <i />
            Control plane connected
          </div>
          <p>Schema {overview?.targetSchemaVersion ?? '—'} · Auto refresh 15s</p>
          <button type="button" onClick={logout}>
            Disconnect ↗
          </button>
        </div>
      </aside>
      <main inert={Boolean(selected || modal)}>
        <header className="topbar">
          <span>
            Platform <b>/</b>{' '}
            {tab === 'tenants'
              ? 'Tenant management'
              : tab === 'cells'
                ? 'Cell management'
                : 'Plan management'}
          </span>
          <button type="button" className="mobile-disconnect" onClick={logout}>
            Disconnect
          </button>
          <span className="operator">
            <i /> Platform operator
          </span>
        </header>
        <div className="content">
          <div className="page-heading">
            <div>
              <span className="eyebrow">CONTROL PLANE</span>
              <h1>
                {tab === 'tenants'
                  ? 'Tenant management'
                  : tab === 'cells'
                    ? 'Cell management'
                    : 'Plan management'}
              </h1>
              <p>
                {tab === 'tenants'
                  ? 'Provision, place and monitor every tenant.'
                  : tab === 'cells'
                    ? 'Register database workers and review tenant assignments and recovery readiness.'
                    : 'Manage reusable subscription plans and entitlements.'}
              </p>
            </div>
            <div className="heading-actions">
              <button type="button" onClick={refresh} disabled={loading}>
                ↻ Refresh
              </button>
              <button
                type="button"
                className="primary"
                onClick={() => {
                  setEditingPlan(undefined);
                  open(tab === 'tenants' ? 'tenant' : tab === 'cells' ? 'cell' : 'plan');
                }}
                disabled={tab === 'tenants' && !provisionReady}
              >
                +{' '}
                {tab === 'tenants'
                  ? 'Create tenant'
                  : tab === 'cells'
                    ? 'Register cell'
                    : 'Create plan'}
              </button>
            </div>
          </div>
          {tab === 'tenants' && overview && (
            <SetupChecklist
              overview={overview}
              worker={worker}
              trialReady={trialReady}
              onRegister={() => {
                setTab('cells');
                open('cell');
              }}
              onPlans={() => {
                setTab('plans');
                const trial = plans.find((plan) => plan.code === 'trial');
                if (trial) {
                  setEditingPlan({ ...trial, isActive: true });
                  open('plan');
                }
              }}
            />
          )}
          {error && (
            <div className="notice error" role="alert">
              Refresh failed: {error}. Displayed data may be stale.
            </div>
          )}
          {message && (
            <div className="notice success" role="status">
              {message}
              <button type="button" onClick={() => setMessage('')} aria-label="Dismiss message">
                ×
              </button>
            </div>
          )}
          <section className="stats" aria-label="Tenant overview">
            {(
              [
                ['Total tenants', overview?.counts.total, 'Across all cells'],
                ['Active tenants', overview?.counts.active, 'Operational and assigned'],
                ['Cold tenants', overview?.counts.cold, 'Stored remotely'],
                ['Needs attention', overview?.counts.attention, 'Transitions or failures'],
              ] as const
            ).map(([label, count, caption]) => (
              <MetricCard key={label} label={label} value={count ?? '—'} detail={caption} />
            ))}
          </section>
          {tab === 'tenants' ? (
            <section className="panel">
              <div className="panel-heading">
                <div>
                  <h2>Tenant directory</h2>
                  <p>Live ownership and lifecycle state</p>
                </div>
                <span className="subtle">{loading ? 'Refreshing…' : `Updated ${updated}`}</span>
              </div>
              <div className="filters">
                <input
                  aria-label="Search tenants"
                  placeholder="Search by name, slug or ID…"
                  value={search}
                  onChange={(e) => {
                    setSearch(e.target.value);
                    setOffset(0);
                  }}
                />
                <select
                  aria-label="Filter state"
                  value={filter}
                  onChange={(e) => {
                    setFilter(e.target.value);
                    setOffset(0);
                  }}
                >
                  <option value="">All states</option>
                  {[
                    'ACTIVE',
                    'COLD',
                    'PROVISIONING',
                    'PREPARING_MOVE',
                    'COPYING',
                    'CUTOVER',
                    'VERIFYING',
                    'RESTORING',
                    'FENCED',
                    'FAILED',
                    'DELETED',
                  ].map((s) => (
                    <option key={s}>{s}</option>
                  ))}
                </select>
              </div>
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>Tenant</th>
                      <th>State</th>
                      <th>Assigned cell</th>
                      <th>Lease</th>
                      <th>Schema</th>
                      <th />
                    </tr>
                  </thead>
                  <tbody>
                    {tenants.map((tenant) => (
                      <tr key={tenant.id}>
                        <td>
                          <button
                            type="button"
                            className="tenant-name"
                            onClick={() => pick(tenant.id)}
                          >
                            {tenant.name}
                          </button>
                          <small>
                            {tenant.slug} · {short(tenant.id)}
                          </small>
                        </td>
                        <td>
                          <Status value={tenant.is_enabled === false ? 'DISABLED' : tenant.state} />
                        </td>
                        <td>
                          {tenant.cell_name ?? 'Unassigned'}
                          <small>{short(tenant.owner_cell)}</small>
                        </td>
                        <td>
                          <span className={tenant.lease_fresh ? 'text-good' : 'subtle'}>
                            {tenant.owner_cell
                              ? tenant.lease_fresh
                                ? '● Fresh'
                                : '○ Expired / missing'
                              : '—'}
                          </span>
                        </td>
                        <td>v{tenant.schema_version}</td>
                        <td>
                          <button
                            type="button"
                            onClick={() => pick(tenant.id)}
                            aria-label={`Manage ${tenant.name}`}
                          >
                            Manage →
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              {!tenants.length && (
                <EmptyState
                  title={
                    loading
                      ? 'Loading tenants…'
                      : search || filter
                        ? 'No tenants match your filters'
                        : 'Your tenant directory is empty'
                  }
                  description="Use Create tenant to set up a workspace and its first administrator."
                />
              )}
              <div className="table-footer">
                <span>
                  {tenants.length
                    ? `Showing ${offset + 1}–${offset + tenants.length}`
                    : 'No results'}
                </span>
                <div>
                  <button
                    type="button"
                    disabled={offset === 0 || loading}
                    onClick={() => setOffset(Math.max(0, offset - 30))}
                  >
                    ← Previous
                  </button>
                  <button
                    type="button"
                    disabled={tenants.length < 30 || loading}
                    onClick={() => setOffset(offset + 30)}
                  >
                    Next →
                  </button>
                </div>
              </div>
            </section>
          ) : tab === 'cells' ? (
            <section className="panel">
              <div className="panel-heading">
                <div>
                  <h2>Cell directory</h2>
                  <p>
                    Database workers store tenant data. Registration uses the address configured on
                    the API service.
                  </p>
                </div>
              </div>
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>Cell</th>
                      <th>Address</th>
                      <th>State</th>
                      <th>Tenants</th>
                      <th>Backup restore</th>
                    </tr>
                  </thead>
                  <tbody>
                    {cells.map((cell) => (
                      <tr key={cell.id}>
                        <td>
                          <strong>{cell.name}</strong>
                          <small>{cell.id}</small>
                          {cell.id === overview?.localCellId && (
                            <span className="local-label">Default database worker</span>
                          )}
                        </td>
                        <td>{cell.address}</td>
                        <td>
                          <Status value={cell.state} />
                        </td>
                        <td>{cell.tenant_count}</td>
                        <td>
                          {cell.hydration_ready ? (
                            <span className="text-good">● Ready</span>
                          ) : (
                            <span className="subtle">Not ready for restore</span>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              {!cells.length && (
                <div className="empty">
                  <strong>Register your first cell</strong>
                  <p>
                    Register the configured worker below, then return to Tenants to create your
                    first workspace.
                  </p>
                </div>
              )}
            </section>
          ) : (
            <section className="panel">
              <div className="panel-heading">
                <h2>Plan catalog</h2>
              </div>
              <div className="table-scroll">
                <table>
                  <thead>
                    <tr>
                      <th>Code</th>
                      <th>Name</th>
                      <th>Grace</th>
                      <th>Status</th>
                      <th />
                    </tr>
                  </thead>
                  <tbody>
                    {plans.map((plan) => (
                      <tr key={plan.code}>
                        <td>{plan.code}</td>
                        <td>{plan.name}</td>
                        <td>{plan.graceDays} days</td>
                        <td>
                          <Status value={plan.isActive ? 'ACTIVE' : 'DISABLED'} />
                        </td>
                        <td>
                          <button
                            type="button"
                            onClick={() => {
                              setEditingPlan(plan);
                              open('plan');
                            }}
                          >
                            Edit
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </section>
          )}
          <div className="architecture-note">
            <span>◇</span> Global PostgreSQL manages identity and placement. Each active tenant owns
            a SQLite database on its cell.
          </div>
        </div>
      </main>
      {selected && (
        <div className="drawer-overlay">
          <aside
            inert={Boolean(modal)}
            className="drawer"
            role="dialog"
            aria-modal="true"
            aria-label="Tenant details"
          >
            <div className="drawer-top">
              <span className="eyebrow">TENANT WORKSPACE</span>
              <button
                type="button"
                aria-label="Close tenant details"
                onClick={() => setSelected(null)}
              >
                ×
              </button>
            </div>
            {detailError ? (
              <div className="notice error" role="alert">
                {detailError}
              </div>
            ) : !t ? (
              <p>Loading tenant…</p>
            ) : (
              <>
                <h2>{t.name}</h2>
                <p className="subtle">{t.slug}</p>
                <Status value={t.state} />
                <dl>
                  <dt>Tenant ID</dt>
                  <dd>{t.id}</dd>
                  <dt>Assigned cell</dt>
                  <dd>{t.cell_name ?? 'Unassigned'}</dd>
                  <dt>Ownership generation</dt>
                  <dd>{t.ownership_generation}</dd>
                  <dt>Schema</dt>
                  <dd>
                    v{t.schema_version} / v{overview?.targetSchemaVersion}
                  </dd>
                  <dt>Timezone</dt>
                  <dd>{t.timezone}</dd>
                  <dt>Lease expires</dt>
                  <dd>{date(t.lease_expires_at)}</dd>
                </dl>
                <h3>Lifecycle actions</h3>
                {t.owner_cell && !cells.find((c) => c.id === t.owner_cell)?.hydration_ready && (
                  <p className="hint">
                    Move and cold storage need a replication-enabled cell with a fresh hydration
                    heartbeat.
                  </p>
                )}
                <div className="action-grid">
                  <button
                    type="button"
                    disabled={
                      t.state !== 'ACTIVE' ||
                      !cells.find((c) => c.id === t.owner_cell)?.hydration_ready
                    }
                    onClick={() => open('move')}
                  >
                    Move to cell
                  </button>
                  <button
                    type="button"
                    disabled={
                      t.state !== 'ACTIVE' ||
                      !cells.find((c) => c.id === t.owner_cell)?.hydration_ready
                    }
                    onClick={() => open('cold')}
                  >
                    Put into cold storage
                  </button>
                  <button
                    type="button"
                    disabled={!['COLD', 'RESTORING'].includes(t.state)}
                    onClick={() => open('wake')}
                  >
                    Wake tenant
                  </button>
                  <button
                    type="button"
                    disabled={['FAILED', 'DELETED'].includes(t.state)}
                    onClick={() => open('timezone')}
                  >
                    Change timezone
                  </button>
                </div>
                <h3>Administrators</h3>
                <a
                  className="panel-link"
                  href={businessAdminUrl()}
                  target="_blank"
                  rel="noreferrer"
                >
                  Open business admin ↗
                </a>
                {detail?.admins.map((a) => (
                  <div className="list-row" key={a.id}>
                    <span>{a.username}</span>
                    <span className="subtle">{a.active ? 'Active' : 'Disabled'}</span>
                  </div>
                ))}
                {detail && !detail.admins.length && (
                  <div className="notice">
                    Finish setup: create the tenant administrator below. Then use that account in
                    the business admin to add venues, devices, staff and products.
                  </div>
                )}
                <button
                  type="button"
                  disabled={['FAILED', 'DELETED'].includes(t.state)}
                  onClick={() => open('admin')}
                >
                  + Create administrator
                </button>
                <h3>License</h3>
                <div className="list-row">
                  <span>Access</span>
                  <Status value={t.is_enabled ? 'ACTIVE' : 'DISABLED'} />
                </div>
                <button
                  type="button"
                  disabled={busy || ['FAILED', 'DELETED'].includes(t.state)}
                  onClick={toggleTenant}
                >
                  {t.is_enabled ? 'Deactivate tenant' : 'Activate tenant'}
                </button>
                <h3>Subscription</h3>
                <p>
                  {detail?.subscription?.planCode ?? 'No plan'} ·{' '}
                  {detail?.subscription?.status ?? '—'}
                </p>
                <p>Expires {date(detail?.subscription?.endsAt ?? null)}</p>
                <button
                  type="button"
                  disabled={busy || ['FAILED', 'DELETED'].includes(t.state)}
                  onClick={() => open('subscription')}
                >
                  Edit plan and expiry
                </button>
                {detail?.licenses.map((l) => (
                  <div className="license" key={l.revision}>
                    <Status value={l.status} />
                    <p>
                      Revision {l.revision} · Valid until {date(l.valid_until)}
                    </p>
                    <small>Grace until {date(l.grace_until)}</small>
                  </div>
                ))}
                <h3>Recent jobs</h3>
                {!detail?.jobs.length && (
                  <p className="subtle">No background jobs for this tenant.</p>
                )}
                {detail?.jobs.map((j) => (
                  <div className="job" key={`${j.kind}-${j.id}`}>
                    <div>
                      <strong>{j.kind}</strong>
                      <Status value={j.state} />
                    </div>
                    <small>
                      {date(j.created_at)} · {short(j.id)}
                    </small>
                    {j.last_error && <p className="text-bad">{j.last_error}</p>}
                    {((j.kind === 'Move' && ['PREPARING_MOVE', 'COPYING'].includes(j.state)) ||
                      (j.kind === 'Cold lifecycle' && j.state === 'SNAPSHOTTING')) && (
                      <button type="button" disabled={busy} onClick={() => cancelJob(j)}>
                        Cancel job
                      </button>
                    )}
                  </div>
                ))}
              </>
            )}
          </aside>
        </div>
      )}
      {modal === 'tenant' && (
        <CreateTenantDialog
          api={api}
          workerName={worker?.name ?? 'Configured database worker'}
          ready={provisionReady}
          onCreated={(id) => {
            pick(id);
            refresh();
          }}
          onBusyChange={setBusy}
          onClose={() => setModal(null)}
        />
      )}
      {modal && modal !== 'tenant' && (
        <div className="modal-overlay">
          <section className="modal" role="dialog" aria-modal="true" aria-labelledby="modal-title">
            <div className="modal-title">
              <h2 id="modal-title">{titles[modal]}</h2>
              <button
                type="button"
                disabled={busy}
                onClick={() => setModal(null)}
                aria-label="Close action"
              >
                ×
              </button>
            </div>
            <form onSubmit={submit}>
              {modal === 'cell' && (
                <>
                  <label>
                    Database worker name
                    <input name="name" required maxLength={100} placeholder="cell-local-01" />
                  </label>
                  <label>
                    Cell ID
                    <input
                      name="id"
                      defaultValue={
                        cells.some((c) => c.id === overview?.localCellId)
                          ? ''
                          : (overview?.localCellId ?? '')
                      }
                      placeholder="UUID (blank generates one)"
                    />
                  </label>
                  <label>
                    Database worker address
                    <input
                      name="address"
                      type="url"
                      required
                      defaultValue={
                        cells.some((cell) => cell.id === overview?.localCellId)
                          ? undefined
                          : (overview?.provisioningAddress ?? undefined)
                      }
                      placeholder={
                        overview?.provisioningMode === 'remote'
                          ? 'http://127.0.0.1:3001'
                          : 'http://127.0.0.1:3000'
                      }
                    />
                  </label>
                  <p className="hint">
                    For the default worker, use the prefilled ID and address from the API service.
                    For another worker, use its ARENA_CELL_ID and a reachable HTTP(S) origin.
                    Registering it does not start the service. Backup restore readiness is only
                    needed for moves and recovery.
                  </p>
                </>
              )}
              {modal === 'admin' && (
                <>
                  <p>
                    Create a new global identity with an administrator membership in {t?.name}.
                    Existing usernames are refused.
                  </p>
                  <label>
                    Username
                    <input name="username" required maxLength={100} autoComplete="off" />
                  </label>
                  <label>
                    Password
                    <input
                      name="password"
                      type="password"
                      minLength={12}
                      maxLength={72}
                      required
                      autoComplete="new-password"
                    />
                  </label>
                  <p className="hint">
                    Use this account in the existing business admin panel. The password is never
                    returned by the portal.
                  </p>
                </>
              )}
              {modal === 'subscription' && (
                <>
                  <label>
                    Plan
                    <select
                      name="planCode"
                      required
                      defaultValue={detail?.subscription?.planCode ?? ''}
                    >
                      <option value="" disabled>
                        Select a plan
                      </option>
                      {plans
                        .filter((plan) => plan.isActive)
                        .map((plan) => (
                          <option key={plan.code} value={plan.code}>
                            {plan.name} ({plan.code})
                          </option>
                        ))}
                    </select>
                  </label>
                  <label>
                    Expires at
                    <input
                      name="endsAt"
                      type="datetime-local"
                      required
                      defaultValue={localDateTime(detail?.subscription?.endsAt ?? null)}
                    />
                  </label>
                  <p className="hint">
                    The selected plan supplies entitlements and grace period. The expiry uses your
                    local timezone.
                  </p>
                </>
              )}
              {modal === 'plan' && (
                <>
                  <label>
                    Plan code
                    <input
                      name="code"
                      required
                      maxLength={64}
                      pattern="[A-Za-z0-9_-]+"
                      defaultValue={editingPlan?.code}
                      disabled={Boolean(editingPlan)}
                    />
                  </label>
                  <label>
                    Name
                    <input name="name" required maxLength={100} defaultValue={editingPlan?.name} />
                  </label>
                  <label>
                    Grace days
                    <input
                      name="graceDays"
                      type="number"
                      required
                      min="0"
                      max="30"
                      defaultValue={editingPlan?.graceDays ?? 7}
                    />
                  </label>
                  <label>
                    Entitlements (JSON object)
                    <textarea
                      name="entitlements"
                      required
                      defaultValue={JSON.stringify(editingPlan?.entitlements ?? {}, null, 2)}
                    />
                  </label>
                  <label>
                    <input
                      name="isActive"
                      type="checkbox"
                      defaultChecked={editingPlan?.isActive ?? true}
                    />{' '}
                    Active for new assignments
                  </label>
                  <p className="hint">
                    Editing a plan affects future assignments. Existing licenses retain their issued
                    entitlements.
                  </p>
                </>
              )}
              {modal === 'timezone' && (
                <>
                  <p>Calendar projections reconcile asynchronously on the owning cell.</p>
                  <label>
                    IANA timezone
                    <input name="timezone" required defaultValue={t?.timezone} />
                  </label>
                </>
              )}
              {modal === 'move' && (
                <>
                  <p>
                    Move {t?.name} from {t?.cell_name}. The current owner performs a verified
                    handoff with a brief write pause.
                  </p>
                  <label>
                    Target cell
                    <select name="targetCell" required defaultValue="">
                      <option value="" disabled>
                        Select another active, ready cell
                      </option>
                      {cells
                        .filter(
                          (c) =>
                            c.state === 'ACTIVE' && c.hydration_ready && c.id !== t?.owner_cell,
                        )
                        .map((c) => (
                          <option key={c.id} value={c.id}>
                            {c.name}
                          </option>
                        ))}
                    </select>
                  </label>
                </>
              )}
              {modal === 'cold' && (
                <>
                  <p>
                    A verified snapshot releases {t?.name}’s lease and removes its local files.
                    Waking requires a running hydration-capable cell.
                  </p>
                  <label>
                    Minimum idle seconds
                    <input
                      name="minimumIdleSeconds"
                      type="number"
                      min="0"
                      max="31536000"
                      required
                      defaultValue="300"
                    />
                  </label>
                </>
              )}
              {modal === 'wake' && (
                <p>
                  Assign {t?.name} to a ready cell and hydrate its verified snapshot. Follow
                  progress in Recent jobs.
                </p>
              )}
              {modalError && (
                <div className="notice error" role="alert">
                  {modalError}
                </div>
              )}
              <div className="modal-footer">
                <button type="button" disabled={busy} onClick={() => setModal(null)}>
                  Cancel
                </button>
                <button type="submit" className="primary" disabled={busy}>
                  {busy ? 'Working…' : titles[modal]}
                </button>
              </div>
            </form>
          </section>
        </div>
      )}
    </div>
  );
}
