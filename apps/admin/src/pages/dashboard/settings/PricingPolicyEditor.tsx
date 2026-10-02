import { Add, DeleteOutline } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Checkbox,
  Chip,
  FormControlLabel,
  IconButton,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import type { PricingPolicy } from '../../../services/pricing-rules';

export default function PricingPolicyEditor({
  value,
  onChange,
  disabled,
}: {
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
}) {
  const [advanced, setAdvanced] = useState(false);
  let policy: PricingPolicy | null = null;
  try {
    const parsed = JSON.parse(value);
    if (
      parsed &&
      Array.isArray(parsed.rules) &&
      typeof parsed.baseRate === 'string' &&
      parsed.rules.every(
        (rule: PricingPolicy['rules'][number] | null) =>
          rule &&
          typeof rule.id === 'string' &&
          typeof rule.name === 'string' &&
          Array.isArray(rule.deviceTypes) &&
          Array.isArray(rule.weekdays) &&
          rule.action &&
          typeof rule.action.value === 'string' &&
          (rule.startTime == null || typeof rule.startTime === 'string') &&
          (rule.endTime == null || typeof rule.endTime === 'string'),
      )
    )
      policy = parsed;
  } catch {
    /* The advanced editor may contain an incomplete draft. */
  }
  const update = (patch: Partial<PricingPolicy>) => {
    if (policy) onChange(JSON.stringify({ ...policy, ...patch }, null, 2));
  };
  return (
    <Box>
      <Stack direction="row" justifyContent="space-between" alignItems="center" sx={{ mb: 2 }}>
        <Box>
          <Typography fontWeight={650}>Policy editor</Typography>
          <Typography variant="caption" color="text.secondary">
            Set your base rate, then add exceptions in priority order.
          </Typography>
        </Box>
        <Button size="small" onClick={() => setAdvanced(!advanced)}>
          {advanced ? 'Visual editor' : 'Advanced JSON'}
        </Button>
      </Stack>
      {advanced || !policy ? (
        <>
          <TextField
            label="Pricing policy JSON"
            value={value}
            onChange={(event) => onChange(event.target.value)}
            disabled={disabled}
            multiline
            minRows={12}
            fullWidth
            spellCheck={false}
            slotProps={{ input: { sx: { fontFamily: 'monospace', fontSize: 12 } } }}
          />
          {!policy && (
            <Alert severity="warning" sx={{ mt: 1 }}>
              Enter a valid policy with a baseRate and rules array to use the visual editor.
            </Alert>
          )}
        </>
      ) : (
        <Stack spacing={2}>
          <Box
            sx={{
              display: 'grid',
              gridTemplateColumns: { xs: '1fr 1fr', md: 'repeat(4, 1fr)' },
              gap: 2,
            }}
          >
            <TextField
              label="Base rate"
              type="number"
              value={policy.baseRate}
              disabled={disabled}
              onChange={(event) => update({ baseRate: event.target.value })}
              slotProps={{ htmlInput: { min: 0, step: 'any' } }}
            />
            <TextField
              label="Minimum price"
              type="number"
              value={policy.minimumPrice ?? ''}
              disabled={disabled}
              onChange={(event) => update({ minimumPrice: event.target.value || null })}
            />
            <TextField
              label="Maximum price (optional)"
              type="number"
              value={policy.maximumPrice ?? ''}
              disabled={disabled}
              onChange={(event) => update({ maximumPrice: event.target.value || null })}
            />
            <TextField
              select
              label="Decimal places"
              value={policy.roundingScale}
              disabled={disabled}
              onChange={(event) => update({ roundingScale: Number(event.target.value) })}
            >
              {[0, 1, 2, 3, 4].map((n) => (
                <MenuItem key={n} value={n}>
                  {n}
                </MenuItem>
              ))}
            </TextField>
          </Box>
          {policy.rules.map((rule, index) => {
            const edit = (patch: Partial<typeof rule>) =>
              update({
                rules: policy.rules.map((item, i) => (i === index ? { ...item, ...patch } : item)),
              });
            return (
              <Box
                key={rule.id}
                sx={{ p: 2.5, border: 1, borderColor: 'divider', borderRadius: 2 }}
              >
                <Stack
                  direction="row"
                  alignItems="center"
                  justifyContent="space-between"
                  sx={{ mb: 2 }}
                >
                  <Stack direction="row" spacing={1} alignItems="center">
                    <Chip label={`Rule ${index + 1}`} />
                    <Typography fontWeight={600}>{rule.name || 'Untitled rule'}</Typography>
                  </Stack>
                  <Tooltip title="Remove rule">
                    <span>
                      <IconButton
                        aria-label={`Remove ${rule.name}`}
                        disabled={disabled}
                        onClick={() =>
                          update({ rules: policy.rules.filter((_, i) => i !== index) })
                        }
                      >
                        <DeleteOutline fontSize="small" />
                      </IconButton>
                    </span>
                  </Tooltip>
                </Stack>
                <Box
                  sx={{
                    display: 'grid',
                    gridTemplateColumns: { xs: '1fr', sm: '1fr 1fr' },
                    gap: 2,
                  }}
                >
                  <TextField
                    label="Rule name"
                    value={rule.name}
                    disabled={disabled}
                    onChange={(event) => edit({ name: event.target.value })}
                  />
                  <TextField
                    label="Priority"
                    type="number"
                    value={rule.priority}
                    disabled={disabled}
                    onChange={(event) => edit({ priority: Number(event.target.value) })}
                  />
                  <TextField
                    select
                    label="Adjustment"
                    value={rule.action.type}
                    disabled={disabled}
                    onChange={(event) =>
                      edit({
                        action: {
                          ...rule.action,
                          type: event.target.value as 'fixed' | 'multiplier',
                        },
                      })
                    }
                  >
                    <MenuItem value="fixed">Fixed price</MenuItem>
                    <MenuItem value="multiplier">Multiply base rate</MenuItem>
                  </TextField>
                  <TextField
                    label={rule.action.type === 'fixed' ? 'Price' : 'Multiplier'}
                    type="number"
                    value={rule.action.value}
                    disabled={disabled}
                    onChange={(event) =>
                      edit({ action: { ...rule.action, value: event.target.value } })
                    }
                  />
                  <TextField
                    label="Starts at (optional)"
                    type="time"
                    value={rule.startTime?.slice(0, 5) ?? ''}
                    disabled={disabled}
                    onChange={(event) =>
                      edit({ startTime: event.target.value ? `${event.target.value}:00` : null })
                    }
                    slotProps={{ inputLabel: { shrink: true } }}
                  />
                  <TextField
                    label="Ends at (optional)"
                    type="time"
                    value={rule.endTime?.slice(0, 5) ?? ''}
                    disabled={disabled}
                    onChange={(event) =>
                      edit({ endTime: event.target.value ? `${event.target.value}:00` : null })
                    }
                    slotProps={{ inputLabel: { shrink: true } }}
                  />
                  <TextField
                    label="Device types"
                    value={rule.deviceTypes.join(', ')}
                    disabled={disabled}
                    onChange={(event) =>
                      edit({
                        deviceTypes: event.target.value.split(',').map((type) => type.trim()),
                      })
                    }
                    onBlur={() => edit({ deviceTypes: rule.deviceTypes.filter(Boolean) })}
                    helperText="Comma-separated. Leave empty for all devices."
                  />
                </Box>
                <Typography
                  variant="caption"
                  color="text.secondary"
                  sx={{ display: 'block', mt: 2 }}
                >
                  Days of the week · leave all unchecked for every day
                </Typography>
                <Box sx={{ display: 'flex', flexWrap: 'wrap' }}>
                  {['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map((day, i) => (
                    <FormControlLabel
                      key={day}
                      label={day}
                      control={
                        <Checkbox
                          size="small"
                          checked={rule.weekdays.includes(i + 1)}
                          disabled={disabled}
                          onChange={(_, checked) =>
                            edit({
                              weekdays: checked
                                ? [...rule.weekdays, i + 1]
                                : rule.weekdays.filter((n) => n !== i + 1),
                            })
                          }
                        />
                      }
                    />
                  ))}
                </Box>
                {(rule.startsAt || rule.endsAt) && (
                  <Alert severity="info" sx={{ mt: 1 }}>
                    This rule also has a date restriction. View and edit it in Advanced JSON.
                  </Alert>
                )}
              </Box>
            );
          })}
          {!policy.rules.length && (
            <Typography color="text.secondary" variant="body2">
              No exceptions. The base rate applies to every session.
            </Typography>
          )}
          <Button
            startIcon={<Add />}
            variant="outlined"
            disabled={disabled}
            onClick={() =>
              update({
                rules: [
                  ...policy.rules,
                  {
                    id: crypto.randomUUID(),
                    name: 'New pricing rule',
                    priority: 100,
                    deviceTypes: [],
                    weekdays: [],
                    action: { type: 'multiplier', value: '1' },
                  },
                ],
              })
            }
            sx={{ alignSelf: 'flex-start' }}
          >
            Add pricing rule
          </Button>
        </Stack>
      )}
    </Box>
  );
}
