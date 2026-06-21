import { formatCurrency, formatTimeAgo } from '@gaming-cafe/utils';
import { ArrowBack as BackIcon } from '@mui/icons-material';
import {
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Divider,
  Paper,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useParams } from 'react-router-dom';
import { formatPaymentSplit } from '../../../containers/sales';
import { getPlanById } from '../../../services/plans/getById';
import { getTransactionById, TransactionType } from '../../../services/transaction/list';

const formatPlanType = (type: string) =>
  type
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');

export default function ProductTransactionDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();

  const { data, isLoading, error } = useQuery({
    queryKey: ['transaction', id],
    queryFn: () => getTransactionById(id as string),
    enabled: !!id,
  });

  const isPlanPurchase = data?.transactionType === TransactionType.PLAN_PURCHASE;

  const { data: plan, isLoading: planLoading } = useQuery({
    queryKey: ['plan', data?.planId],
    queryFn: () => getPlanById(data?.planId as string),
    enabled: isPlanPurchase && !!data?.planId,
  });

  if (isLoading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  if (error || !data) {
    return (
      <Box sx={{ p: 4 }}>
        <Typography color="error">Failed to load transaction details.</Typography>
        <Button
          startIcon={<BackIcon />}
          onClick={() => navigate('/product-transactions')}
          sx={{ mt: 2 }}
        >
          Back to Transactions
        </Button>
      </Box>
    );
  }

  const lineItems = data.lineItems || [];
  const lineItemsTotal = lineItems.reduce((sum, item) => sum + item.quantity * item.unitPrice, 0);
  const planPrice = plan ? parseFloat(plan.price) : data.amount;
  const backPath = isPlanPurchase ? '/plan-transactions' : '/product-transactions';
  const backLabel = isPlanPurchase ? 'Back to Plan Transactions' : 'Back to Transactions';

  return (
    <Box sx={{ p: 3, maxWidth: 900, mx: 'auto' }}>
      <Button startIcon={<BackIcon />} onClick={() => navigate(backPath)} sx={{ mb: 3 }}>
        {backLabel}
      </Button>

      <Card sx={{ mb: 3 }}>
        <CardContent>
          <Box
            sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', mb: 2 }}
          >
            <Typography variant="h5" fontWeight={600}>
              Transaction Details
            </Typography>
            <Chip
              label={data.paymentStatus.charAt(0).toUpperCase() + data.paymentStatus.slice(1)}
              color={
                data.paymentStatus === 'completed'
                  ? 'success'
                  : data.paymentStatus === 'pending'
                    ? 'warning'
                    : 'default'
              }
              size="small"
            />
          </Box>

          <Divider sx={{ mb: 2 }} />

          <Box sx={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 2 }}>
            <Box>
              <Typography variant="caption" color="text.secondary">
                Transaction ID
              </Typography>
              <Typography variant="body2" fontFamily="monospace">
                {data.id}
              </Typography>
            </Box>
            <Box>
              <Typography variant="caption" color="text.secondary">
                Date
              </Typography>
              <Typography variant="body2">{formatTimeAgo(data.transactionDate)}</Typography>
            </Box>
            <Box>
              <Typography variant="caption" color="text.secondary">
                Amount
              </Typography>
              <Typography variant="body1" fontWeight={600}>
                {formatCurrency(data.amount, 'INR')}
              </Typography>
            </Box>
            <Box>
              <Typography variant="caption" color="text.secondary">
                Payment Method
              </Typography>
              <Typography variant="body2">{data.paymentMethod}</Typography>
            </Box>
            <Box sx={{ gridColumn: '1 / -1' }}>
              <Typography variant="caption" color="text.secondary">
                Payment Split
              </Typography>
              <Typography variant="body2">
                {formatPaymentSplit({
                  paymentMethod: data.paymentMethod,
                  amount: data.amount,
                  cashAmount: data.cashAmount,
                  onlineAmount: data.onlineAmount,
                })}
              </Typography>
            </Box>
            {data.notes && (
              <Box sx={{ gridColumn: '1 / -1' }}>
                <Typography variant="caption" color="text.secondary">
                  Notes
                </Typography>
                <Typography variant="body2">{data.notes}</Typography>
              </Box>
            )}
          </Box>
        </CardContent>
      </Card>

      <Card>
        <CardContent>
          <Typography variant="h6" fontWeight={600} sx={{ mb: 2 }}>
            Line Items ({isPlanPurchase ? (plan ? 1 : 0) : lineItems.length})
          </Typography>

          {isPlanPurchase ? (
            planLoading ? (
              <Box sx={{ display: 'flex', justifyContent: 'center', py: 3 }}>
                <CircularProgress size={24} />
              </Box>
            ) : plan ? (
              <TableContainer component={Paper} variant="outlined">
                <Table size="small">
                  <TableHead>
                    <TableRow>
                      <TableCell>Plan</TableCell>
                      <TableCell>Type</TableCell>
                      <TableCell align="right">Unit Price</TableCell>
                      <TableCell align="right">Qty</TableCell>
                      <TableCell align="right">Subtotal</TableCell>
                    </TableRow>
                  </TableHead>
                  <TableBody>
                    <TableRow>
                      <TableCell>{plan.name}</TableCell>
                      <TableCell>{formatPlanType(plan.planType)}</TableCell>
                      <TableCell align="right">{formatCurrency(planPrice, 'INR')}</TableCell>
                      <TableCell align="right">1</TableCell>
                      <TableCell align="right">{formatCurrency(planPrice, 'INR')}</TableCell>
                    </TableRow>
                    <TableRow>
                      <TableCell colSpan={4} align="right">
                        <Typography fontWeight={600}>Total</Typography>
                      </TableCell>
                      <TableCell align="right">
                        <Typography fontWeight={600}>{formatCurrency(planPrice, 'INR')}</Typography>
                      </TableCell>
                    </TableRow>
                  </TableBody>
                </Table>
              </TableContainer>
            ) : (
              <Typography variant="body2" color="text.secondary">
                Plan details are unavailable for this transaction.
              </Typography>
            )
          ) : lineItems.length === 0 ? (
            <Typography variant="body2" color="text.secondary">
              No line items recorded for this transaction (created before line item tracking was
              enabled).
            </Typography>
          ) : (
            <TableContainer component={Paper} variant="outlined">
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>Product</TableCell>
                    <TableCell>SKU</TableCell>
                    <TableCell align="right">Unit Price</TableCell>
                    <TableCell align="right">Qty</TableCell>
                    <TableCell align="right">Subtotal</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {lineItems.map((item) => (
                    <TableRow key={item.id}>
                      <TableCell>{item.productName}</TableCell>
                      <TableCell>{item.productSku || '—'}</TableCell>
                      <TableCell align="right">{formatCurrency(item.unitPrice, 'INR')}</TableCell>
                      <TableCell align="right">{item.quantity}</TableCell>
                      <TableCell align="right">
                        {formatCurrency(item.quantity * item.unitPrice, 'INR')}
                      </TableCell>
                    </TableRow>
                  ))}
                  <TableRow>
                    <TableCell colSpan={4} align="right">
                      <Typography fontWeight={600}>Total</Typography>
                    </TableCell>
                    <TableCell align="right">
                      <Typography fontWeight={600}>
                        {formatCurrency(lineItemsTotal, 'INR')}
                      </Typography>
                    </TableCell>
                  </TableRow>
                </TableBody>
              </Table>
            </TableContainer>
          )}
        </CardContent>
      </Card>
    </Box>
  );
}
