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
}: {
  organizationId: string;
  locationId?: string;
}) {
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
  });
  const versionsQuery = useQuery({
    queryKey: ['pricing-rule-versions', organizationId, selectedSetId],
    queryFn: () => listPricingRuleVersions(organizationId, selectedSetId),
    enabled: Boolean(selectedSetId),
  });

  useEffect(() => {
    if (!selectedSetId && setsQuery.data?.[0]) setSelectedSetId(setsQuery.data[0].id);
  }, [selectedSetId, setsQuery.data]);

  useEffect(() => {
    const version = versionsQuery.data?.[0];
    if (version) {
      setSelectedVersionId(version.id);
      setPolicyText(JSON.stringify(version.policy, null, 2));
      setSimulation(null);
    }
  }, [versionsQuery.data]);

  const selectedVersion = useMemo(
    () => versionsQuery.data?.find((version) => version.id === selectedVersionId),
    [selectedVersionId, versionsQuery.data],
  );

  const parsePolicy = (): PricingPolicy => {
    const parsed = JSON.parse(policyText) as PricingPolicy;
    if (!parsed.baseRate || !Array.isArray(parsed.rules))
      throw new Error('Policy requires baseRate and rules');
    return parsed;
  };

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

  return (
    <Card variant="outlined">
      <CardHeader
        title="Governed pricing rules"
        subheader="Draft, validate, simulate and publish deterministic venue pricing"
      />
      <Divider />
      <CardContent>
        <Stack spacing={2}>
          <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}>
            <FormControl size="small" fullWidth>
              <InputLabel>Rule set</InputLabel>
              <Select
                label="Rule set"
                value={selectedSetId}
                onChange={(event) => setSelectedSetId(event.target.value)}
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
              label="New rule set name"
              value={name}
              onChange={(event) => setName(event.target.value)}
              fullWidth
            />
            <Button
              variant="outlined"
              disabled={busy}
              onClick={() =>
                run(async () => {
                  const created = await createPricingRuleSet(organizationId, {
                    locationId,
                    name,
                    policy: parsePolicy(),
                  });
                  setSelectedSetId(created.ruleSet.id);
                  await refresh(created.ruleSet.id);
                  toastUtils.success('Pricing rule set draft created');
                })
              }
            >
              Create
            </Button>
          </Stack>

          {selectedSetId && (
            <FormControl size="small" fullWidth>
              <InputLabel>Version</InputLabel>
              <Select
                label="Version"
                value={selectedVersionId}
                onChange={(event) => {
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

          <TextField
            label="Typed pricing policy (JSON)"
            value={policyText}
            onChange={(event) => setPolicyText(event.target.value)}
            multiline
            minRows={12}
            fullWidth
            spellCheck={false}
          />

          <Stack direction={{ xs: 'column', md: 'row' }} spacing={1} alignItems="center">
            <Button
              variant="outlined"
              disabled={busy || !selectedSetId}
              onClick={() =>
                run(async () => {
                  const version = await createPricingRuleVersion(
                    organizationId,
                    selectedSetId,
                    parsePolicy(),
                  );
                  await refresh();
                  setSelectedVersionId(version.id);
                  toastUtils.success('New pricing draft created');
                })
              }
            >
              Save as draft
            </Button>
            <Button
              variant="outlined"
              disabled={busy || !selectedVersionId || selectedVersion?.status === 'published'}
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
              disabled={busy || !selectedVersionId}
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
            <TextField
              size="small"
              label="Publish at (optional)"
              type="datetime-local"
              value={effectiveAt}
              onChange={(event) => setEffectiveAt(event.target.value)}
              slotProps={{ inputLabel: { shrink: true } }}
              sx={{ minWidth: 220 }}
            />
            <Button
              variant="contained"
              disabled={busy || !selectedVersionId || !selectedVersion?.simulationHash}
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
              Publish / schedule
            </Button>
            {busy && <CircularProgress size={20} />}
          </Stack>

          {selectedVersion && (
            <Box>
              <Chip size="small" label={selectedVersion.status} />
              {selectedVersion.status === 'superseded' && selectedVersion.simulationHash && (
                <Button
                  size="small"
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
        </Stack>
      </CardContent>
    </Card>
  );
}
