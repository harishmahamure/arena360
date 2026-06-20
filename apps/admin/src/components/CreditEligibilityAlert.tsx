import { Alert, Box, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { PaymentMethodValues } from '../containers/transactions/schemas/transaction-schema';
import { getPlayerCredit } from '../services/credit';

interface CreditEligibilityAlertProps {
  playerId?: string;
  paymentMethod?: string;
  purchaseAmount?: number;
}

export default function CreditEligibilityAlert({
  playerId,
  paymentMethod,
  purchaseAmount = 0,
}: CreditEligibilityAlertProps) {
  const isCreditPayment = paymentMethod === PaymentMethodValues.CREDIT;

  const { data, isLoading, error } = useQuery({
    queryKey: ['player-credit', playerId],
    queryFn: () => getPlayerCredit(playerId as string),
    enabled: isCreditPayment && !!playerId,
  });

  if (!isCreditPayment) return null;

  if (!playerId) {
    return (
      <Alert severity="warning" sx={{ mb: 2 }}>
        Select a player to check credit eligibility.
      </Alert>
    );
  }

  if (isLoading) {
    return (
      <Alert severity="info" sx={{ mb: 2 }}>
        Checking credit eligibility…
      </Alert>
    );
  }

  if (error) {
    return (
      <Alert severity="error" sx={{ mb: 2 }}>
        {error instanceof Error ? error.message : 'Unable to verify credit eligibility'}
      </Alert>
    );
  }

  const summary = data?.summary;
  if (!summary) return null;

  const formatCurrency = (amount: number) =>
    new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);

  if (!summary.creditEnabled || summary.creditLimit <= 0) {
    return (
      <Alert severity="error" sx={{ mb: 2 }}>
        Credit is not enabled for this player. Ask an admin to set a credit limit.
      </Alert>
    );
  }

  if (purchaseAmount > summary.available + 0.001) {
    return (
      <Alert severity="error" sx={{ mb: 2 }}>
        Exceeds available credit ({formatCurrency(summary.available)}). Outstanding:{' '}
        {formatCurrency(summary.outstanding)} / Limit: {formatCurrency(summary.creditLimit)}
      </Alert>
    );
  }

  return (
    <Alert severity="success" sx={{ mb: 2 }}>
      <Box>
        <Typography variant="body2" fontWeight={600}>
          Credit available: {formatCurrency(summary.available)}
        </Typography>
        <Typography variant="caption" color="text.secondary">
          Outstanding {formatCurrency(summary.outstanding)} of {formatCurrency(summary.creditLimit)}{' '}
          limit
        </Typography>
      </Box>
    </Alert>
  );
}
