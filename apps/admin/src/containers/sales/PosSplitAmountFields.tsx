import { CurrencyField } from '@gaming-cafe/ui';
import { formatCurrency } from '@gaming-cafe/utils';
import { Alert, Grid, Typography } from '@mui/material';

export interface PosSplitAmountFieldsProps {
  cashAmount: string;
  onlineAmount: string;
  onCashChange: (value: string) => void;
  onOnlineChange: (value: string) => void;
  totalAmount: number;
  helperText?: string;
}

export function PosSplitAmountFields({
  cashAmount,
  onlineAmount,
  onCashChange,
  onOnlineChange,
  totalAmount,
  helperText = 'Must add up to the sale total',
}: PosSplitAmountFieldsProps) {
  const cash = Number.parseFloat(cashAmount) || 0;
  const online = Number.parseFloat(onlineAmount) || 0;
  const entered = cashAmount.trim().length > 0 || onlineAmount.trim().length > 0;
  const remaining = totalAmount - cash - online;
  const mismatch = entered && Math.abs(remaining) > 0.01;

  return (
    <>
      <Grid container spacing={2} sx={{ mb: 1 }}>
        <Grid size={{ xs: 6 }}>
          <CurrencyField
            label="Cash Amount"
            value={cashAmount}
            onChange={(e) => onCashChange(e.target.value)}
            fullWidth
            error={mismatch}
            helperText={helperText}
          />
        </Grid>
        <Grid size={{ xs: 6 }}>
          <CurrencyField
            label="Online Amount"
            value={onlineAmount}
            onChange={(e) => onOnlineChange(e.target.value)}
            fullWidth
            error={mismatch}
            helperText={helperText}
          />
        </Grid>
      </Grid>
      <Typography
        variant="caption"
        color={mismatch ? 'error.main' : 'text.secondary'}
        display="block"
        sx={{ mb: 2 }}
      >
        Sale total: {formatCurrency(totalAmount, 'INR')}
        {entered &&
          ` · Split total: ${formatCurrency(cash + online, 'INR')}${
            mismatch ? ` · Remaining: ${formatCurrency(Math.abs(remaining), 'INR')}` : ''
          }`}
      </Typography>
      {mismatch && (
        <Alert severity="warning" sx={{ mb: 2 }}>
          Cash and online amounts must add up to {formatCurrency(totalAmount, 'INR')}.
        </Alert>
      )}
    </>
  );
}
