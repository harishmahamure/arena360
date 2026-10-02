import { toastUtils } from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  CardHeader,
  Chip,
  CircularProgress,
  Divider,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { GuidedForm, GuidedStep } from '../../../components/GuidedForm';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import {
  createPricingRuleSet,
  createPricingRuleVersion,
  listPricingRuleSets,
  listPricingRuleVersions,
  type PricingPolicy,
  type PricingSimulationResult,
  publishPricingRuleVersion,
  rollbackPricingRuleVersion,
  simulatePricingRuleVersion,
  validatePricingRuleVersion,
} from '../../../services/pricing-rules';

import PricingPolicyEditor from './PricingPolicyEditor';
import { parsePricingPolicy } from './pricingPolicy';

const DEFAULT_POLICY: PricingPolicy = {
  baseRate: '60',
  roundingScale: 2,
  minimumPrice: '0',
  rules: [
    {
      id: 'night-rate',
      name: 'Night rate',
      priority: 100,
      deviceTypes: [],
      weekdays: [],
      startTime: '23:00:00',
      endTime: '08:00:00',
      action: { type: 'multiplier', value: '1.25' },
    },
  ],
};

export default function PricingRulesPanel({
  organizationId,
  locationId,
  readOnly = false,
  onDirtyChange,
}: {
  organizationId: string;
  locationId?: string;
  readOnly?: boolean;
  onDirtyChange?: (dirty: boolean) => void;
}) {
  const { can } = usePermissions();
  const canRead = can(Permission.RulesRead);
  const canEdit = can(Permission.RulesEdit) && !readOnly;
  const canPublish = can(Permission.RulesPublish);
  const queryClient = useQueryClient();
  const [selectedSetId, setSelectedSetId] = useState('');
  const [selectedVersionId, setSelectedVersionId] = useState('');
  const [name, setName] = useState('Venue pricing');
  const [deviceType, setDeviceType] = useState('PC');
  const [effectiveAt, setEffectiveAt] = useState('');
  const [policyText, setPolicyText] = useState(JSON.stringify(DEFAULT_POLICY, null, 2));
  const [simulation, setSimulation] = useState<PricingSimulationResult | null>(null);
  const [busy, setBusy] = useState(false);

  const setsQuery = useQuery({
    queryKey: ['pricing-rule-sets', organizationId, locationId],
    queryFn: () => listPricingRuleSets(organizationId, locationId),
    enabled: canRead,
  });
  const versionsQuery = useQuery({
    queryKey: ['pricing-rule-versions', organizationId, selectedSetId],
    queryFn: () => listPricingRuleVersions(organizationId, selectedSetId),
    enabled: canRead && Boolean(selectedSetId),
  });

  useEffect(() => {
    if (!selectedSetId && setsQuery.data?.[0]) setSelectedSetId(setsQuery.data[0].id);
  }, [selectedSetId, setsQuery.data]);

  useEffect(() => {
    const version = versionsQuery.data?.[0];
    if (version && !selectedVersionId) {
      setSelectedVersionId(version.id);
      setPolicyText(JSON.stringify(version.policy, null, 2));
      setSimulation(null);
    }
  }, [versionsQuery.data, selectedVersionId]);

  const selectedVersion = useMemo(
    () => versionsQuery.data?.find((version) => version.id === selectedVersionId),
    [selectedVersionId, versionsQuery.data],
  );

  const hasUnsavedEdits =
    policyText !== JSON.stringify(selectedVersion?.policy ?? DEFAULT_POLICY, null, 2);
  useEffect(() => {
    onDirtyChange?.(hasUnsavedEdits);
    return () => onDirtyChange?.(false);
  }, [hasUnsavedEdits, onDirtyChange]);

  const parsePolicy = () => parsePricingPolicy(policyText);

  const refresh = async (setId = selectedSetId) => {
    await queryClient.invalidateQueries({ queryKey: ['pricing-rule-sets', organizationId] });
    if (setId) {
      await queryClient.invalidateQueries({
        queryKey: ['pricing-rule-versions', organizationId, setId],
      });
    }
  };

  const run = async (action: () => Promise<void>) => {
    setBusy(true);
    try {
      await action();
    } catch (error) {
      toastUtils.error(error instanceof Error ? error.message : 'Pricing rule action failed');
    } finally {
      setBusy(false);
    }
  };

  if (!canRead)
    return <Alert severity="info">Your account does not have access to pricing policies.</Alert>;

  return (
    <Card variant="outlined">
      <CardHeader
        title="Pricing policies"
        subheader="Create a draft, validate it, test the price, then publish when you’re ready."
      />
      <Divider />
      <CardContent>
        <Stack spacing={3}>
          {(setsQuery.isError || versionsQuery.isError) && (
            <Alert
              severity="error"
              action={
                <Button
                  onClick={() => {
                    void setsQuery.refetch();
                    if (selectedSetId) void versionsQuery.refetch();
                  }}
                >
                  Retry
                </Button>
              }
            >
              Pricing policies could not be loaded.
            </Alert>
          )}
          {setsQuery.isLoading && (
            <CircularProgress size={22} aria-label="Loading pricing policies" />
          )}
          {hasUnsavedEdits && (
            <Alert severity="warning">
              You have unsaved policy edits. Save a new draft before validating, simulating, or
              publishing.
            </Alert>
          )}
          <GuidedForm
            busy={busy}
            reviewDescription="Review the saved draft and simulation before publishing. Drafts are stored separately and do not change live pricing."
            actions={
              <Button
                variant="contained"
                disabled={
                  busy ||
                  !canPublish ||
                  hasUnsavedEdits ||
                  !selectedVersionId ||
                  !selectedVersion?.simulationHash ||
                  selectedVersion.status === 'published'
                }
                onClick={() =>
                  run(async () => {
                    await publishPricingRuleVersion(
                      organizationId,
                      selectedSetId,
                      selectedVersionId,
                      effectiveAt ? new Date(effectiveAt).toISOString() : undefined,
                    );
                    await refresh();
                    toastUtils.success(
                      effectiveAt ? 'Pricing policy scheduled' : 'Pricing policy published',
                    );
                  })
                }
              >
                Publish policy
              </Button>
            }
          >
            <GuidedStep
              title="Choose policy"
              validate={() =>
                !selectedVersionId ? 'Create or select a policy draft to continue.' : undefined
              }
            >
              <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}>
                <FormControl size="small" fullWidth>
                  <InputLabel id="pricing-policy-label" shrink>
                    Policy
                  </InputLabel>
                  <Select
                    label="Policy"
                    labelId="pricing-policy-label"
                    displayEmpty
                    renderValue={(id) =>
                      setsQuery.data?.find((set) => set.id === id)?.name ??
                      'Create your first policy'
                    }
                    value={selectedSetId}
                    onChange={(event) => {
                      if (hasUnsavedEdits && !window.confirm('Discard unsaved policy edits?'))
                        return;
                      setSelectedSetId(event.target.value);
                      setSelectedVersionId('');
                      setSimulation(null);
                    }}
                    disabled={busy}
                  >
                    {(setsQuery.data ?? []).map((set) => (
                      <MenuItem key={set.id} value={set.id}>
                        {set.name}
                      </MenuItem>
                    ))}
                  </Select>
                </FormControl>
                <TextField
                  size="small"
                  label="New policy name"
                  disabled={!canEdit || busy}
                  value={name}
                  onChange={(event) => setName(event.target.value)}
                  fullWidth
                />
                <Button
                  variant="outlined"
                  disabled={busy || !canEdit || !name.trim() || setsQuery.isError}
                  onClick={() =>
                    run(async () => {
                      const created = await createPricingRuleSet(organizationId, {
                        locationId,
                        name,
                        policy: parsePolicy(),
                      });
                      setSelectedSetId(created.ruleSet.id);
                      setSelectedVersionId(created.version.id);
                      setPolicyText(JSON.stringify(created.version.policy, null, 2));
                      setSimulation(null);
                      await refresh(created.ruleSet.id);
                      toastUtils.success('Pricing rule set draft created');
                    })
                  }
                >
                  Create draft
                </Button>
              </Stack>
              {selectedSetId && (
                <FormControl size="small" fullWidth>
                  <InputLabel id="pricing-version-label">Version</InputLabel>
                  <Select
                    label="Version"
                    labelId="pricing-version-label"
                    value={selectedVersionId}
                    disabled={busy}
                    onChange={(event) => {
                      if (hasUnsavedEdits && !window.confirm('Discard unsaved policy edits?'))
                        return;
                      const id = event.target.value;
                      setSelectedVersionId(id);
                      const version = versionsQuery.data?.find((item) => item.id === id);
                      if (version) setPolicyText(JSON.stringify(version.policy, null, 2));
                      setSimulation(null);
                    }}
                  >
                    {(versionsQuery.data ?? []).map((version) => (
                      <MenuItem key={version.id} value={version.id}>
                        Version {version.version} — {version.status}
                      </MenuItem>
                    ))}
                  </Select>
                </FormControl>
              )}
            </GuidedStep>
            <GuidedStep
              title="Rates & rules"
              validate={() => {
                try {
                  parsePolicy();
                } catch (error) {
                  return error instanceof Error ? error.message : 'Check the policy values.';
                }
                return hasUnsavedEdits
                  ? 'Save your changes as a draft before continuing.'
                  : undefined;
              }}
            >
              <PricingPolicyEditor
                value={policyText}
                onChange={setPolicyText}
                disabled={!canEdit || busy}
              />
              <Button
                variant="outlined"
                disabled={busy || !canEdit || !selectedSetId}
                onClick={() =>
                  run(async () => {
                    const version = await createPricingRuleVersion(
                      organizationId,
                      selectedSetId,
                      parsePolicy(),
                    );
                    await refresh();
                    setSelectedVersionId(version.id);
                    setPolicyText(JSON.stringify(version.policy, null, 2));
                    setSimulation(null);
                    toastUtils.success('New pricing draft created');
                  })
                }
              >
                Save as draft
              </Button>
            </GuidedStep>
            <GuidedStep
              title="Validate & simulate"
              validate={() =>
                !selectedVersion?.simulationHash || hasUnsavedEdits
                  ? 'Validate and simulate the saved draft before continuing.'
                  : undefined
              }
            >
              <Button
                variant="outlined"
                disabled={
                  busy ||
                  !canEdit ||
                  hasUnsavedEdits ||
                  !selectedVersionId ||
                  selectedVersion?.status === 'published'
                }
                onClick={() =>
                  run(async () => {
                    await validatePricingRuleVersion(
                      organizationId,
                      selectedSetId,
                      selectedVersionId,
                    );
                    await refresh();
                    toastUtils.success('Pricing policy validated');
                  })
                }
              >
                Validate
              </Button>
              <TextField
                size="small"
                label="Device type"
                value={deviceType}
                onChange={(event) => setDeviceType(event.target.value)}
                sx={{ minWidth: 150 }}
              />
              <Button
                variant="outlined"
                disabled={busy || hasUnsavedEdits || !selectedVersionId}
                onClick={() =>
                  run(async () => {
                    const result = await simulatePricingRuleVersion(
                      organizationId,
                      selectedSetId,
                      selectedVersionId,
                      { locationId, deviceType, at: new Date().toISOString() },
                    );
                    setSimulation(result);
                    await refresh();
                  })
                }
              >
                Simulate now
              </Button>
              {simulation && (
                <Alert severity="info">
                  <Typography fontWeight={700}>
                    {simulation.baseRate} → {simulation.finalPrice} {simulation.currency}
                  </Typography>
                  <Typography variant="caption">Timezone: {simulation.timezone}</Typography>
                  {simulation.trace.map((step) => (
                    <Typography key={step.ruleId} variant="body2">
                      {step.ruleName}: {step.before} → {step.after} ({step.action})
                    </Typography>
                  ))}
                </Alert>
              )}
            </GuidedStep>
            <GuidedStep
              title="Publication"
              validate={() =>
                effectiveAt && !Number.isFinite(new Date(effectiveAt).getTime())
                  ? 'Choose a valid publication date.'
                  : undefined
              }
            >
              <TextField
                size="small"
                label="Publish at (optional)"
                type="datetime-local"
                value={effectiveAt}
                onChange={(event) => setEffectiveAt(event.target.value)}
                slotProps={{ inputLabel: { shrink: true } }}
                sx={{ minWidth: 220 }}
              />
              {selectedVersion && (
                <Box>
                  <Chip size="small" label={selectedVersion.status} />
                  {selectedVersion.status === 'superseded' && selectedVersion.simulationHash && (
                    <Button
                      size="small"
                      disabled={busy || !canPublish || hasUnsavedEdits}
                      sx={{ ml: 1 }}
                      onClick={() =>
                        run(async () => {
                          await rollbackPricingRuleVersion(
                            organizationId,
                            selectedSetId,
                            selectedVersion.id,
                          );
                          await refresh();
                          toastUtils.success('Previous pricing version republished');
                        })
                      }
                    >
                      Roll back to this version
                    </Button>
                  )}
                </Box>
              )}
            </GuidedStep>
          </GuidedForm>
        </Stack>
      </CardContent>
    </Card>
  );
}
