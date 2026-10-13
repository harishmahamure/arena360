import { Permission } from '@gaming-cafe/contracts';
import { PageHeader, PageShell, WizardProgress } from '@gaming-cafe/ui';
import { isApiError } from '@gaming-cafe/utils';
import {
  Alert,
  Button,
  Card,
  CardContent,
  Chip,
  LinearProgress,
  Stack,
  Typography,
} from '@mui/material';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useRef } from 'react';
import { Link, useNavigate } from 'react-router-dom';
import { useOnboarding } from '../../../hooks/useOnboarding';
import { usePermissions } from '../../../hooks/usePermissions';
import { panelClaims } from '../../../lib/authSession';
import {
  completeOnboarding,
  type OnboardingSnapshot,
  type OnboardingStep,
  saveOnboarding,
} from '../../../services/onboarding';
import {
  OperatorForm,
  SettingsForm,
  type SetupFormHandle,
  StationsForm,
  VenueForm,
} from './SetupForms';

const steps: { key: OnboardingStep; label: string }[] = [
  { key: 'venue', label: 'Venue and location' },
  { key: 'settings', label: 'Operating settings' },
  { key: 'stations', label: 'Stations and pricing' },
  { key: 'review', label: 'Readiness review' },
];

function saveError(error: unknown): string {
  if (isApiError(error)) {
    if (error.message === 'ONBOARDING_REVISION_CONFLICT') {
      return 'Another administrator updated setup. Reload the saved progress before continuing.';
    }
    if (error.message === 'ONBOARDING_NOT_READY') {
      return 'Your venue changed since the last check. Review the remaining items before completing setup.';
    }
  }
  return error instanceof Error ? error.message : 'Setup could not be saved. Please try again.';
}

function Readiness({
  data,
  onEdit,
  disabled = false,
}: {
  data: OnboardingSnapshot;
  onEdit?: (step: OnboardingStep) => void;
  disabled?: boolean;
}) {
  const { can } = usePermissions();
  const links: Record<string, Permission> = {
    venue: Permission.LocationsRead,
    settings: Permission.SettingsRead,
    stations: Permission.DevicesRead,
    pricing: Permission.PlansRead,
    operator: Permission.AccessRead,
  };
  return (
    <Stack gap={1}>
      {data.readiness.checks.map((check) => (
        <Stack
          key={check.key}
          direction={{ xs: 'column', sm: 'row' }}
          gap={1}
          alignItems={{ xs: 'flex-start', sm: 'center' }}
        >
          <Chip
            label={check.ready ? 'Ready' : 'Needs attention'}
            color={check.ready ? 'success' : 'default'}
            size="small"
          />
          <Typography sx={{ flex: 1 }}>{check.label}</Typography>
          {!check.ready && onEdit && check.key !== 'operator' && (
            <Button
              disabled={disabled}
              onClick={() =>
                onEdit(
                  check.key === 'venue'
                    ? 'venue'
                    : check.key === 'settings'
                      ? 'settings'
                      : 'stations',
                )
              }
            >
              Edit
            </Button>
          )}
          {!check.ready && !onEdit && can(links[check.key] ?? Permission.SettingsRead) && (
            <Button component={Link} to={check.actionPath}>
              Review
            </Button>
          )}
        </Stack>
      ))}
      {data.readiness.locations.map((location) => (
        <Typography key={location.id} variant="body2" color="text.secondary">
          {location.name} · {location.timezone} · {location.currency} · {location.stationCount}{' '}
          stations · {location.compatiblePlanCount} compatible plans
        </Typography>
      ))}
    </Stack>
  );
}

export default function OnboardingPage() {
  const query = useOnboarding();
  const client = useQueryClient();
  const navigate = useNavigate();
  const { can } = usePermissions();
  const form = useRef<SetupFormHandle>(null);
  const organizationId = panelClaims()?.tenantId ?? '';
  const save = useMutation({
    mutationFn: async (input: {
      step: OnboardingStep;
      exit?: boolean;
      reviewSettings?: boolean;
    }) => {
      const expectedRevision = query.data?.progress.revision ?? 0;
      await form.current?.save(
        input.exit ||
          steps.findIndex((step) => step.key === input.step) <=
            steps.findIndex((step) => step.key === query.data?.progress.currentStep),
      );
      const result = await saveOnboarding({
        currentStep: input.step,
        expectedRevision,
        reviewSettings: input.reviewSettings,
      });
      return result;
    },
    onSuccess: (data, input) => {
      client.setQueryData(query.queryKey, data);
      if (input.exit) navigate('/');
    },
  });
  const complete = useMutation({
    mutationFn: async () => {
      const expectedRevision = query.data?.progress.revision ?? 0;
      await form.current?.save();
      return completeOnboarding(expectedRevision);
    },
    onSuccess: (data) => client.setQueryData(query.queryKey, data),
  });
  const reload = async () => {
    const result = await query.refetch();
    if (!result.isError) {
      save.reset();
      complete.reset();
    }
  };
  if (!query.canRead)
    return (
      <Alert severity="error">
        Organization administrator access with settings permission is required.
      </Alert>
    );
  if (query.isPending) return <LinearProgress aria-label="Loading venue setup" />;
  if (!query.data)
    return (
      <Alert severity="error" action={<Button onClick={() => void reload()}>Retry</Button>}>
        Could not load setup. {query.error?.message}
      </Alert>
    );
  const data = query.data;
  if (data.progress.completedAt) {
    return (
      <PageShell>
        <PageHeader
          title={data.readiness.ready ? 'Your venue is ready' : 'Venue setup is complete'}
          description={
            data.readiness.ready
              ? 'Setup is complete. You can now open a shift and start serving players.'
              : 'Your saved setup is complete. Review the changes below before starting operations.'
          }
        />
        <Card variant="outlined">
          <CardContent>
            <Stack gap={3}>
              <Alert severity={data.readiness.ready ? 'success' : 'warning'}>
                {data.readiness.ready
                  ? 'Venue setup completed. Your locations, settings, stations, and plans are saved.'
                  : 'Venue configuration has changed since setup was completed. Resolve the remaining readiness checks before starting operations.'}
              </Alert>
              <Readiness data={data} />
              <Typography>
                Start operations by choosing a location and confirming the opening cash to start a
                shift. Then add a player, sell a plan, and start their session.
              </Typography>
              <Stack direction="row" gap={2}>
                {can(Permission.ShiftsWrite) ? (
                  <Button
                    component={Link}
                    to="/shift/setup"
                    variant="contained"
                    disabled={!data.readiness.ready}
                  >
                    Start operations
                  </Button>
                ) : (
                  <Typography>
                    Ask your assigned counter operator to sign in and start a shift.
                  </Typography>
                )}
                <Button component={Link} to="/">
                  Open dashboard
                </Button>
              </Stack>
            </Stack>
          </CardContent>
        </Card>
      </PageShell>
    );
  }
  const index = steps.findIndex((step) => step.key === data.progress.currentStep);
  const pending = save.isPending || complete.isPending || query.isFetching;
  const formProps = { organizationId, writable: query.canWrite, disabled: pending, formRef: form };
  const otherChecksReady = data.readiness.checks
    .filter((check) => check.key !== 'operator')
    .every((check) => check.ready);
  return (
    <PageShell>
      <PageHeader
        title="Set up your venue"
        description="Save each step and return whenever you need. We’ll check the venue before you start operations."
      />
      <Stack gap={3}>
        {!query.canWrite && (
          <Alert severity="info">
            You can review setup. Settings write permission is required to save progress or finish.
          </Alert>
        )}
        <WizardProgress
          titles={steps.map((step) => step.label)}
          activeStep={index}
          disabled={pending || !query.canWrite}
          onBackTo={(step) => save.mutate({ step: steps[step]?.key ?? 'venue' })}
        />
        <Card variant="outlined">
          <CardContent>
            <Stack gap={2}>
              {index === 0 && <VenueForm key="venue" {...formProps} />}
              {index === 1 && <SettingsForm key="settings" {...formProps} />}
              {index === 2 && <StationsForm key="stations" {...formProps} data={data} />}
              {index === 3 && (
                <>
                  <Typography variant="h6" component="h2">
                    Review your venue
                  </Typography>
                  <Typography color="text.secondary">
                    Check your saved location, settings, station, and plan. Complete setup saves
                    operator access and checks the latest venue configuration.
                  </Typography>
                  <Readiness
                    data={data}
                    disabled={pending || !query.canWrite}
                    onEdit={(step) => save.mutate({ step, exit: false })}
                  />
                  <OperatorForm key="review" {...formProps} data={data} />
                </>
              )}
            </Stack>
          </CardContent>
        </Card>
        {(save.error || complete.error || query.error) && (
          <Alert
            severity="error"
            action={
              <Button disabled={pending} onClick={() => void reload()}>
                Reload saved progress
              </Button>
            }
          >
            {saveError(save.error ?? complete.error ?? query.error)}
          </Alert>
        )}
        <Stack direction={{ xs: 'column', sm: 'row' }} gap={2}>
          <Button
            disabled={pending || !query.canWrite}
            onClick={() => save.mutate({ step: data.progress.currentStep, exit: true })}
          >
            Save and exit
          </Button>
          {index > 0 && (
            <Button
              disabled={pending || !query.canWrite}
              onClick={() => save.mutate({ step: steps[index - 1]?.key ?? 'venue' })}
            >
              Back
            </Button>
          )}
          {index < steps.length - 1 ? (
            <Button
              variant="contained"
              disabled={pending || !query.canWrite}
              onClick={() =>
                save.mutate({
                  step: steps[index + 1]?.key ?? 'review',
                  reviewSettings: index === 1,
                })
              }
            >
              {pending ? 'Saving…' : 'Save and continue'}
            </Button>
          ) : (
            <Button
              variant="contained"
              disabled={pending || !query.canWrite || !otherChecksReady}
              onClick={() => complete.mutate()}
            >
              {complete.isPending ? 'Completing…' : 'Complete setup'}
            </Button>
          )}
        </Stack>
        <Typography variant="body2" color="text.secondary">
          {data.progress.updatedAt
            ? `Progress saved ${new Date(data.progress.updatedAt).toLocaleString()}`
            : 'Progress will be saved when you continue or exit.'}
        </Typography>
      </Stack>
    </PageShell>
  );
}
