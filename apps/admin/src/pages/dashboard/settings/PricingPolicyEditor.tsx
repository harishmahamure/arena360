import { deviceTypeOptions } from '@gaming-cafe/contracts';
import { Add, ArrowDownward, ArrowUpward, DeleteOutline } from '@mui/icons-material';
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
import { ProductCategory } from '../../../services/product/list';

const CATEGORIES = Object.values(ProductCategory);

export default function PricingPolicyEditor({
  value,
  onChange,
  disabled,
  products = [],
}: {
  value: string;
  onChange: (value: string) => void;
  disabled: boolean;
  products?: Array<{ id: string; name: string }>;
}) {
  const productName = (id: string) => products.find((p) => p.id === id)?.name ?? id;
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
            Choose price or credit deduction rules, then add exceptions in priority order.
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
              helperText="Preview price. Checkout uses the selected plan’s price."
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
          {[...policy.rules]
            .sort(
              (left, right) => right.priority - left.priority || left.id.localeCompare(right.id),
            )
            .map((rule, index, orderedRules) => {
              const move = (direction: -1 | 1) => {
                const destination = index + direction;
                if (destination < 0 || destination >= orderedRules.length) return;
                const reordered = [...orderedRules];
                const adjacent = reordered[destination];
                if (!adjacent) return;
                reordered[index] = adjacent;
                reordered[destination] = rule;
                const precedence = new Map(
                  reordered.map((item, position) => [item.id, (reordered.length - position) * 100]),
                );
                update({
                  rules: policy.rules.map((item) => ({
                    ...item,
                    priority: precedence.get(item.id) ?? item.priority,
                  })),
                });
              };
              const forProducts = rule.target === 'products';
              const forDeduction = rule.target === 'deduction';
              const deviceOptions = [
                ...deviceTypeOptions,
                ...rule.deviceTypes
                  .filter((value) => !deviceTypeOptions.some((item) => item.value === value))
                  .map((value) => ({ value, label: value })),
              ];
              const edit = (patch: Partial<typeof rule>) =>
                update({
                  rules: policy.rules.map((item) =>
                    item.id === rule.id ? { ...item, ...patch } : item,
                  ),
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
                    <Stack direction="row" spacing={0.5}>
                      <Tooltip title="Increase precedence">
                        <span>
                          <IconButton
                            aria-label={`Increase precedence for ${rule.name}`}
                            disabled={disabled || index === 0}
                            onClick={() => move(-1)}
                            size="small"
                          >
                            <ArrowUpward fontSize="small" />
                          </IconButton>
                        </span>
                      </Tooltip>
                      <Tooltip title="Decrease precedence">
                        <span>
                          <IconButton
                            aria-label={`Decrease precedence for ${rule.name}`}
                            disabled={disabled || index === orderedRules.length - 1}
                            onClick={() => move(1)}
                            size="small"
                          >
                            <ArrowDownward fontSize="small" />
                          </IconButton>
                        </span>
                      </Tooltip>
                      <Tooltip title="Remove rule">
                        <span>
                          <IconButton
                            aria-label={`Remove ${rule.name}`}
                            disabled={disabled}
                            onClick={() =>
                              update({ rules: policy.rules.filter((item) => item.id !== rule.id) })
                            }
                          >
                            <DeleteOutline fontSize="small" />
                          </IconButton>
                        </span>
                      </Tooltip>
                    </Stack>
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
                      select
                      label="Applies to"
                      value={rule.target ?? 'sessions'}
                      disabled={disabled}
                      onChange={(event) =>
                        edit(
                          event.target.value === 'products'
                            ? { target: 'products', deviceTypes: [] }
                            : {
                                target: event.target.value as 'sessions' | 'deduction',
                                productIds: [],
                                categories: [],
                                action: { type: 'multiplier', value: '1' },
                              },
                        )
                      }
                    >
                      <MenuItem value="sessions">Gaming plan price</MenuItem>
                      <MenuItem value="deduction">Credit deduction speed</MenuItem>
                      <MenuItem value="products">Product sales</MenuItem>
                    </TextField>
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
                      <MenuItem value="fixed">
                        {forDeduction ? 'Set deduction speed' : 'Fixed price'}
                      </MenuItem>
                      <MenuItem value="multiplier">
                        {forDeduction
                          ? 'Multiply plan deduction speed'
                          : forProducts
                            ? 'Multiply product price'
                            : 'Multiply plan price'}
                      </MenuItem>
                    </TextField>
                    <TextField
                      label={
                        forDeduction
                          ? rule.action.type === 'fixed'
                            ? 'Credit minutes per minute'
                            : 'Deduction multiplier'
                          : rule.action.type === 'fixed'
                            ? 'Price'
                            : 'Multiplier'
                      }
                      helperText={
                        forDeduction
                          ? '1.25× consumes 75 credit minutes in 60 minutes at normal plan speed. Applies when a session starts, including existing plan balances.'
                          : undefined
                      }
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
                    {forProducts ? (
                      <>
                        <TextField
                          select
                          label="Products"
                          value={rule.productIds ?? []}
                          disabled={disabled}
                          onChange={(event) =>
                            edit({ productIds: event.target.value as unknown as string[] })
                          }
                          slotProps={{
                            select: {
                              multiple: true,
                              renderValue: (ids) => (ids as string[]).map(productName).join(', '),
                            },
                          }}
                          helperText="Leave empty to match every product."
                        >
                          {products.map((product) => (
                            <MenuItem key={product.id} value={product.id}>
                              {product.name}
                            </MenuItem>
                          ))}
                        </TextField>
                        <TextField
                          select
                          label="Categories"
                          value={rule.categories ?? []}
                          disabled={disabled}
                          onChange={(event) =>
                            edit({ categories: event.target.value as unknown as string[] })
                          }
                          slotProps={{
                            select: {
                              multiple: true,
                              renderValue: (values) => (values as string[]).join(', '),
                            },
                          }}
                          helperText="A product matches if it is listed or in a chosen category."
                        >
                          {CATEGORIES.map((category) => (
                            <MenuItem key={category} value={category}>
                              {category}
                            </MenuItem>
                          ))}
                        </TextField>
                      </>
                    ) : (
                      <TextField
                        select
                        label="Device types"
                        value={rule.deviceTypes}
                        disabled={disabled}
                        onChange={(event) =>
                          edit({
                            deviceTypes:
                              typeof event.target.value === 'string'
                                ? event.target.value.split(',')
                                : event.target.value,
                          })
                        }
                        slotProps={{
                          select: {
                            multiple: true,
                            renderValue: (values) =>
                              (values as string[])
                                .map(
                                  (value) =>
                                    deviceOptions.find((item) => item.value === value)?.label ??
                                    value,
                                )
                                .join(', '),
                          },
                        }}
                        helperText="Select one or more device types. Leave empty for all devices."
                      >
                        {deviceOptions.map((option) => (
                          <MenuItem key={option.value} value={option.value}>
                            <Checkbox
                              checked={rule.deviceTypes.includes(option.value)}
                              size="small"
                            />
                            {option.label}
                          </MenuItem>
                        ))}
                      </TextField>
                    )}
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
