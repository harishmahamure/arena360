import { formatCurrency } from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Divider,
  Grid,
  Link,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Link as RouterLink, useNavigate, useParams } from 'react-router-dom';
import { formatPaymentSplit } from '../../../containers/sales';
import { PaymentMethodValues } from '../../../containers/transactions/schemas/transaction-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getCreditSettlementById } from '../../../services/credit';
import { TransactionType } from '../../../services/transactions/list';
import { formatDisplayDateTime } from '../../../utils/date';

const formatTransactionType = (type: string) =>
  type
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');

export default function CreditSettlementDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { can } = usePermissions();

  const {
    data: settlement,
    isLoading,
    error,
  } = useQuery({
    queryKey: ['credit-settlement', id],
    queryFn: () => getCreditSettlementById(id as string),
    enabled: !!id,
  });

  if (!can(Permission.CreditRead)) {
    return (
      <Box sx={{ p: 3 }}>
        <Typography>You do not have permission to view credit settlements.</Typography>
      </Box>
    );
  }

  if (isLoading) {
    return (
      <Box sx={{ p: 3 }}>
        <Typography>Loading settlement…</Typography>
      </Box>
    );
  }

  if (error || !settlement) {
    return (
      <Box sx={{ p: 3 }}>
        <Alert severity="error" sx={{ mb: 2 }}>
          {error instanceof Error ? error.message : 'Settlement not found'}
        </Alert>
        <Button onClick={() => navigate('/credit/settlements')}>Back to history</Button>
      </Box>
    );
  }

  const paymentLabel =
    settlement.paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
      ? formatPaymentSplit({
          paymentMethod: settlement.paymentMethod,
          amount: settlement.amount,
          cashAmount: settlement.cashAmount,
          onlineAmount: settlement.onlineAmount,
        })
      : settlement.paymentMethod;

  return (
    <Box sx={{ p: { xs: 2, md: 4 } }}>
      <Button sx={{ mb: 2 }} onClick={() => navigate('/credit/settlements')}>
        Back to settlement history
      </Button>

      <Typography variant="h5" fontWeight={700} sx={{ mb: 1 }}>
        Credit settlement
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
        Settled {formatDisplayDateTime(settlement.settledAt)}
      </Typography>

      <Card variant="outlined" sx={{ mb: 3 }}>
        <CardContent>
          <Grid container spacing={2}>
            <Grid size={{ xs: 12, sm: 6, md: 4 }}>
              <Typography variant="caption" color="text.secondary">
                Player
              </Typography>
              <Typography variant="body1" fontWeight={600}>
                {settlement.playerUsername}
              </Typography>
            </Grid>
            <Grid size={{ xs: 12, sm: 6, md: 4 }}>
              <Typography variant="caption" color="text.secondary">
                Amount
              </Typography>
              <Typography variant="body1" fontWeight={600}>
                {formatCurrency(settlement.amount, 'INR')}
              </Typography>
            </Grid>
            <Grid size={{ xs: 12, sm: 6, md: 4 }}>
              <Typography variant="caption" color="text.secondary">
                Payment
              </Typography>
              <Typography variant="body1">{paymentLabel}</Typography>
            </Grid>
            <Grid size={{ xs: 12, sm: 6, md: 4 }}>
              <Typography variant="caption" color="text.secondary">
                Settled by
              </Typography>
              <Typography variant="body1">{settlement.settledByUsername}</Typography>
            </Grid>
            <Grid size={{ xs: 12, sm: 6, md: 4 }}>
              <Typography variant="caption" color="text.secondary">
                Settlement ID
              </Typography>
              <Typography variant="body2" sx={{ fontFamily: 'monospace' }}>
                {settlement.id}
              </Typography>
            </Grid>
            {settlement.notes && (
              <Grid size={{ xs: 12 }}>
                <Typography variant="caption" color="text.secondary">
                  Notes
                </Typography>
                <Typography variant="body1">{settlement.notes}</Typography>
              </Grid>
            )}
          </Grid>
        </CardContent>
      </Card>

      <Typography variant="h6" fontWeight={600} sx={{ mb: 1 }}>
        Applied to transactions
      </Typography>
      <Divider sx={{ mb: 2 }} />

      <Card variant="outlined">
        <Table size="small">
          <TableHead>
            <TableRow>
              <TableCell>Original date</TableCell>
              <TableCell>Type</TableCell>
              <TableCell align="right">Original</TableCell>
              <TableCell align="right">Applied</TableCell>
              <TableCell align="right">Remaining</TableCell>
              <TableCell>Transaction</TableCell>
            </TableRow>
          </TableHead>
          <TableBody>
            {settlement.items.map((item) => {
              const detailPath =
                item.transactionType === TransactionType.PLAN_PURCHASE
                  ? `/plan-transactions/${item.transactionId}`
                  : `/product-transactions/${item.transactionId}`;

              return (
                <TableRow key={item.transactionId}>
                  <TableCell>{formatDisplayDateTime(item.transactionDate)}</TableCell>
                  <TableCell>
                    <Chip
                      label={formatTransactionType(item.transactionType)}
                      size="small"
                      variant="outlined"
                    />
                  </TableCell>
                  <TableCell align="right">{formatCurrency(item.originalAmount, 'INR')}</TableCell>
                  <TableCell align="right">
                    <Typography component="span" fontWeight={600}>
                      {formatCurrency(item.amountApplied, 'INR')}
                    </Typography>
                  </TableCell>
                  <TableCell align="right">{formatCurrency(item.remainingAfter, 'INR')}</TableCell>
                  <TableCell>
                    <Link component={RouterLink} to={detailPath} underline="hover">
                      View
                    </Link>
                  </TableCell>
                </TableRow>
              );
            })}
          </TableBody>
        </Table>
      </Card>
    </Box>
  );
}
