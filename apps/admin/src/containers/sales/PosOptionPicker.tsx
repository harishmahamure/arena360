import {
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Radio,
  Stack,
  Typography,
} from '@mui/material';
import { useEffect, useState } from 'react';
import type { ProductOptionGroup } from '../../services/product/recipe';

export interface PickedOptions {
  optionIds: string[];
  names: string[];
  priceDelta: number;
}

/** Same rules the server enforces: required groups need a pick, single groups allow one. */
export function missingRequiredGroup(groups: ProductOptionGroup[], selected: string[]) {
  return groups.find(
    (group) =>
      group.required && !group.options.some((option) => option.id && selected.includes(option.id)),
  );
}

export function PosOptionPicker({
  productName,
  basePrice,
  groups,
  onCancel,
  onConfirm,
}: {
  productName: string;
  basePrice: number;
  groups: ProductOptionGroup[] | null;
  onCancel: () => void;
  onConfirm: (picked: PickedOptions) => void;
}) {
  const [selected, setSelected] = useState<string[]>([]);
  useEffect(() => {
    if (groups) setSelected([]);
  }, [groups]);

  const options = (groups ?? []).flatMap((group) => group.options);
  const chosen = options.filter((option) => option.id && selected.includes(option.id));
  const priceDelta = chosen.reduce((sum, option) => sum + option.priceDelta, 0);
  const missing = groups ? missingRequiredGroup(groups, selected) : undefined;

  const toggle = (group: ProductOptionGroup, optionId: string, checked: boolean) => {
    const groupIds = group.options.flatMap((option) => (option.id ? [option.id] : []));
    setSelected((current) => {
      const others = group.multiple ? current : current.filter((id) => !groupIds.includes(id));
      return checked ? [...others, optionId] : others.filter((id) => id !== optionId);
    });
  };

  return (
    <Dialog open={groups !== null} onClose={onCancel} fullWidth maxWidth="xs">
      <DialogTitle>{productName}</DialogTitle>
      <DialogContent dividers>
        <Stack spacing={2}>
          {(groups ?? []).map((group) => (
            <Stack key={group.id ?? group.name}>
              <Typography fontWeight={600}>
                {group.name}
                <Typography
                  component="span"
                  variant="caption"
                  color="text.secondary"
                  sx={{ ml: 1 }}
                >
                  {group.required ? 'Required' : 'Optional'}
                  {group.multiple ? ' · pick any' : ' · pick one'}
                </Typography>
              </Typography>
              {group.options.map((option) => {
                const id = option.id ?? '';
                const Control = group.multiple ? Checkbox : Radio;
                return (
                  <FormControlLabel
                    key={id}
                    control={
                      <Control
                        checked={selected.includes(id)}
                        onChange={(_, checked) => toggle(group, id, checked)}
                        onClick={
                          group.multiple || !selected.includes(id)
                            ? undefined
                            : () => toggle(group, id, false)
                        }
                      />
                    }
                    label={
                      option.priceDelta
                        ? `${option.name} (${option.priceDelta > 0 ? '+' : '−'}₹${Math.abs(option.priceDelta).toFixed(2)})`
                        : option.name
                    }
                  />
                );
              })}
            </Stack>
          ))}
        </Stack>
      </DialogContent>
      <DialogActions sx={{ justifyContent: 'space-between', px: 3 }}>
        <Typography fontWeight={600}>₹{Math.max(0, basePrice + priceDelta).toFixed(2)}</Typography>
        <Stack direction="row" spacing={1}>
          <Button onClick={onCancel}>Cancel</Button>
          <Button
            variant="contained"
            disabled={Boolean(missing)}
            title={missing ? `Choose ${missing.name}` : undefined}
            onClick={() =>
              onConfirm({
                optionIds: chosen.flatMap((option) => (option.id ? [option.id] : [])),
                names: chosen.map((option) => option.name),
                priceDelta,
              })
            }
          >
            Add to cart
          </Button>
        </Stack>
      </DialogActions>
    </Dialog>
  );
}
