import { AccountBalance, CallSplit, CreditCard, Payments } from '@mui/icons-material';
import { Card, CardActionArea, CardContent, Grid, Typography } from '@mui/material';
import {
  PaymentMethodValues,
  paymentMethodOptions,
} from '../transactions/schemas/transaction-schema';

const PAYMENT_TILE_ICONS: Record<string, typeof Payments> = {
  [PaymentMethodValues.CASH]: Payments,
  [PaymentMethodValues.ONLINE]: CreditCard,
  [PaymentMethodValues.SPLIT_PAYMENT]: CallSplit,
  [PaymentMethodValues.CREDIT]: AccountBalance,
};

export interface PosPaymentTilesProps {
  value: string;
  onChange: (method: string) => void;
  disabled?: boolean;
}

export function PosPaymentTiles({ value, onChange, disabled = false }: PosPaymentTilesProps) {
  return (
    <Grid container spacing={1.5} sx={{ mb: 2 }}>
      {paymentMethodOptions.map((option) => {
        const Icon = PAYMENT_TILE_ICONS[option.value] ?? Payments;
        const selected = value === option.value;
        return (
          <Grid key={option.value} size={{ xs: 12, sm: 6 }}>
            <Card
              variant="outlined"
              sx={{
                borderWidth: selected ? 2 : 1,
                borderColor: selected ? 'primary.main' : 'divider',
                bgcolor: selected ? 'action.selected' : 'background.paper',
                cursor: disabled ? 'default' : 'pointer',
                opacity: disabled ? 0.6 : 1,
              }}
              onClick={disabled ? undefined : () => onChange(option.value)}
            >
              <CardActionArea disabled={disabled} sx={{ minHeight: 56 }}>
                <CardContent
                  sx={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 1.5,
                    py: 1.5,
                    '&:last-child': { pb: 1.5 },
                  }}
                >
                  <Icon color={selected ? 'primary' : 'action'} />
                  <Typography variant="body2" fontWeight={selected ? 600 : 500}>
                    {option.label}
                  </Typography>
                </CardContent>
              </CardActionArea>
            </Card>
          </Grid>
        );
      })}
    </Grid>
  );
}
