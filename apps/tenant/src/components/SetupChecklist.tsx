import type { Cell, Overview } from '../api';

export default function SetupChecklist({
  overview,
  worker,
  trialReady,
  onRegister,
  onPlans,
}: {
  overview: Overview;
  worker?: Cell;
  trialReady: boolean;
  onRegister: () => void;
  onPlans: () => void;
}) {
  const registered = worker?.state === 'ACTIVE';
  const ready = overview.canProvision && registered && trialReady;
  return (
    <section className="setup-checklist" aria-label="Tenant creation checklist">
      <div className="setup-checklist-heading">
        <h2>{ready ? 'Ready to create tenants' : 'Before you create your first tenant'}</h2>
        <p>
          {ready
            ? 'Create a workspace, add its administrator, then open the staff panel.'
            : 'Complete the checks below. Your new tenant will use a trial plan.'}
        </p>
      </div>
      <ol>
        <li className={overview.canProvision ? 'complete' : ''}>
          <span className="setup-number">{overview.canProvision ? '✓' : '1'}</span>
          <div>
            <strong>Database service</strong>
            <p>
              {overview.canProvision
                ? 'Tenant creation is configured.'
                : 'The API needs a configured database worker. Ask your deployment administrator to connect it.'}
            </p>
          </div>
        </li>
        <li className={registered ? 'complete' : ''}>
          <span className="setup-number">{registered ? '✓' : '2'}</span>
          <div>
            <strong>Worker registration</strong>
            <p>
              {registered
                ? `${worker.name} is registered for new tenants.`
                : worker
                  ? `${worker.name} is ${worker.state.toLowerCase()}. An active worker is required.`
                  : 'Register the worker configured on your API service.'}
            </p>
            {!worker && overview.canProvision && (
              <button type="button" onClick={onRegister}>
                Register database worker
              </button>
            )}
          </div>
        </li>
        <li className={trialReady ? 'complete' : ''}>
          <span className="setup-number">{trialReady ? '✓' : '3'}</span>
          <div>
            <strong>Starting plan</strong>
            <p>
              {trialReady
                ? 'The Trial plan is active.'
                : 'An active Trial plan (code: trial) is required.'}
            </p>
            {!trialReady && (
              <button type="button" onClick={onPlans}>
                Review trial plan
              </button>
            )}
          </div>
        </li>
      </ol>
    </section>
  );
}
