import { type FieldConfig, FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { formatCurrency } from '@gaming-cafe/utils';
import {
  Box,
  Card,
  CardContent,
  Chip,
  type ChipProps,
  Divider,
  GridLegacy as Grid,
  Paper,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  type UpdatePlanTransactionStatusFormData,
  updatePlanTransactionStatusSchema,
} from '../../../containers/transactions/schemas/transaction-schema';
import { getTransactionById } from '../../../services/transactions/getById';
import { PaymentStatus } from '../../../services/transactions/list';
import { updateTransaction } from '../../../services/transactions/update';

const getStatusColor = (status: PaymentStatus): ChipProps['color'] => {
  switch (status) {
    case PaymentStatus.COMPLETED:
      return 'success';
    case PaymentStatus.PENDING:
      return 'warning';
    case PaymentStatus.FAILED:
      return 'error';
    case PaymentStatus.REFUNDED:
      return 'info';
    default:
      return 'default';
  }
};

const formatPlanType = (type: string) => {
  return type
    .split('_')
    .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
    .join(' ');
};

export default function ViewPlanTransactionPage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { data: transaction, isLoading } = useQuery({
    queryKey: ['transaction', id],
    queryFn: () => getTransactionById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30, // 30 seconds
    refetchOnWindowFocus: true,
    refetchOnMount: true,
    refetchOnReconnect: true,
  });

  const {
    amount,
    paymentMethod,
    paymentStatus,
    transactionDate,
    player: { username } = {},
    plan: { name: planName = '', planType = '', price = 0 } = {},
    notes,
  } = transaction?.data || {};

  const updateStatusFormFields: FieldConfig<UpdatePlanTransactionStatusFormData>[] = [
    {
      name: 'notes',
      label: 'Additional Notes',
      type: 'textarea',
      placeholder: 'Add notes about the status update...',
      fullWidth: true,
      rows: 3,
      helperText: 'Optional notes about the status change (max 500 characters)',
    },
  ];

  const handleSubmit = async (data: UpdatePlanTransactionStatusFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.paymentStatus) {
      setError('Payment status is required');
      setIsSubmitting(false);
      return;
    }

    try {
      await updateTransaction(id as string, {
        paymentStatus: data.paymentStatus as PaymentStatus,
        notes: data.notes || undefined,
      });

      setSuccess('Transaction status updated successfully!');

      // Navigate back to plan transactions list after a short delay
      setTimeout(() => {
        navigate('/plan-transactions');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update transaction');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/plan-transactions');
  };

  if (isLoading) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <FormSkeleton />
      </Paper>
    );
  }

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <Paper elevation={0} sx={{ p: 4 }}>
        <Box sx={{ mb: 4 }}>
          <Typography variant="h4" fontWeight={600} gutterBottom>
            Transaction Details
          </Typography>
          <Typography variant="body2" color="text.secondary">
            View and update transaction information
          </Typography>
        </Box>

        {/* Transaction Information */}
        <Card variant="outlined" sx={{ mb: 4 }}>
          <CardContent>
            <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
              Transaction Information
            </Typography>
            <Divider sx={{ mb: 2 }} />
            <Grid container spacing={2}>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Transaction ID
                </Typography>
                <Typography variant="body1" sx={{ fontFamily: 'monospace', fontSize: '0.875rem' }}>
                  {id}
                </Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Amount
                </Typography>
                <Typography variant="h6" color="primary">
                  {formatCurrency(parseFloat(amount || '0'), 'INR')}
                </Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Payment Method
                </Typography>
                <Typography variant="body1">{paymentMethod?.toUpperCase() || 'N/A'}</Typography>
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Payment Status
                </Typography>
                <Chip
                  label={
                    paymentStatus
                      ? paymentStatus.charAt(0).toUpperCase() + paymentStatus.slice(1)
                      : 'N/A'
                  }
                  color={getStatusColor(paymentStatus as PaymentStatus)}
                  size="small"
                  sx={{ mt: 0.5 }}
                />
              </Grid>
              <Grid item xs={12} md={6}>
                <Typography variant="body2" color="text.secondary">
                  Transaction Date
                </Typography>
                <Typography variant="body1">
                  {transactionDate
                    ? new Date(transactionDate).toLocaleDateString('en-US', {
                        year: 'numeric',
                        month: 'long',
                        day: 'numeric',
                        hour: '2-digit',
                        minute: '2-digit',
                      })
                    : 'N/A'}
                </Typography>
              </Grid>
            </Grid>
          </CardContent>
        </Card>

        {/* Player Information */}
        {username && (
          <Card variant="outlined" sx={{ mb: 4 }}>
            <CardContent>
              <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
                Player Information
              </Typography>
              <Divider sx={{ mb: 2 }} />
              <Grid container spacing={2}>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Name
                  </Typography>
                  <Typography variant="body1">{username || 'N/A'}</Typography>
                </Grid>
              </Grid>
            </CardContent>
          </Card>
        )}

        {/* Plan Information */}
        {planName && (
          <Card variant="outlined" sx={{ mb: 4 }}>
            <CardContent>
              <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
                Plan Information
              </Typography>
              <Divider sx={{ mb: 2 }} />
              <Grid container spacing={2}>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Plan Name
                  </Typography>
                  <Typography variant="body1">{planName || 'N/A'}</Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Plan Type
                  </Typography>
                  <Typography variant="body1">
                    {planType ? formatPlanType(planType) : 'N/A'}
                  </Typography>
                </Grid>
                <Grid item xs={12} md={6}>
                  <Typography variant="body2" color="text.secondary">
                    Plan Price
                  </Typography>
                  <Typography variant="body1">
                    {formatCurrency(parseFloat(price?.toString() || '0'), 'INR')}
                  </Typography>
                </Grid>
              </Grid>
            </CardContent>
          </Card>
        )}

        {/* Notes */}
        {notes && (
          <Card variant="outlined" sx={{ mb: 4 }}>
            <CardContent>
              <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
                Notes
              </Typography>
              <Divider sx={{ mb: 2 }} />
              <Typography variant="body1">{notes}</Typography>
            </CardContent>
          </Card>
        )}

        {/* Update Status Form */}
        <Card variant="outlined">
          <CardContent>
            <Typography variant="h6" gutterBottom sx={{ mb: 2 }}>
              Update Transaction Status
            </Typography>
            <Divider sx={{ mb: 3 }} />
            <FormBuilder<UpdatePlanTransactionStatusFormData>
              fields={updateStatusFormFields}
              schema={updatePlanTransactionStatusSchema}
              defaultValues={{
                paymentStatus: paymentStatus as PaymentStatus,
                notes: '',
              }}
              mode="edit"
              onSubmit={handleSubmit}
              onCancel={handleCancel}
              loading={isSubmitting}
              error={error}
              success={success}
              showCancel
              submitLabel="Update Status"
              cancelLabel="Back to List"
              buttonAlign="right"
              spacing={3}
            />
          </CardContent>
        </Card>
      </Paper>
    </Box>
  );
}
