import { Grid, InputAdornment, TextField } from '@mui/material';

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
        <TextField
          label="Cash Amount"
          type="number"
          value={cashAmount}
          onChange={(e) => onCashChange(e.target.value)}
          fullWidth
          helperText={helperText}
          InputProps={{
            startAdornment: <InputAdornment position="start">₹</InputAdornment>,
          }}
        />
      </Grid>
      <Grid size={{ xs: 6 }}>
        <TextField
          label="Online Amount"
          type="number"
          value={onlineAmount}
          onChange={(e) => onOnlineChange(e.target.value)}
          fullWidth
          helperText={helperText}
          InputProps={{
            startAdornment: <InputAdornment position="start">₹</InputAdornment>,
          }}
        />
      </Grid>
    </Grid>
  );
}
