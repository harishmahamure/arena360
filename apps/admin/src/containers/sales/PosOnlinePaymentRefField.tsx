import { TextField } from '@mui/material';

export interface PosOnlinePaymentRefFieldProps {
  value: string;
  onChange: (value: string) => void;
  disabled?: boolean;
  error?: boolean;
  helperText?: string;
}

export function PosOnlinePaymentRefField({
  value,
  onChange,
  disabled = false,
  error = false,
  helperText = "Enter the last 4 digits from the customer's UPI receipt",
}: PosOnlinePaymentRefFieldProps) {
  return (
    <TextField
      label="UPI UTR ID (last 4 digits)"
      value={value}
      onChange={(e) => onChange(e.target.value.replace(/\D/g, '').slice(0, 4))}
      fullWidth
      required
      disabled={disabled}
      error={error}
      helperText={helperText}
      inputProps={{
        inputMode: 'numeric',
        maxLength: 4,
        pattern: '[0-9]*',
        autoComplete: 'off',
      }}
      sx={{ mb: 2 }}
    />
  );
}
