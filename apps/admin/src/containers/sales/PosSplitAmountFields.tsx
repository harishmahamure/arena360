import { CurrencyField } from '@gaming-cafe/ui';
import { Grid } from '@mui/material';

export interface PosSplitAmountFieldsProps {
  cashAmount: string;
  onlineAmount: string;
  onCashChange: (value: string) => void;
  onOnlineChange: (value: string) => void;
  helperText?: string;
}

export function PosSplitAmountFields({
  cashAmount,
  onlineAmount,
  onCashChange,
  onOnlineChange,
  helperText = 'Must add up to the sale total',
}: PosSplitAmountFieldsProps) {
  return (
    <Grid container spacing={2} sx={{ mb: 2 }}>
      <Grid size={{ xs: 6 }}>
        <CurrencyField
          label="Cash Amount"
          value={cashAmount}
          onChange={(e) => onCashChange(e.target.value)}
          fullWidth
          helperText={helperText}
        />
      </Grid>
      <Grid size={{ xs: 6 }}>
        <CurrencyField
          label="Online Amount"
          value={onlineAmount}
          onChange={(e) => onOnlineChange(e.target.value)}
          fullWidth
          helperText={helperText}
        />
      </Grid>
    </Grid>
  );
}
