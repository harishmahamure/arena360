import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreatePlanTransactionFormData,
  createPlanTransactionDefaultValues,
  createPlanTransactionSchema,
  type PaymentMethodType,
  PaymentMethodValues,
  paymentMethodOptions,
} from '../../../../src/containers/transactions/schemas/transaction-schema';
import { getPlans } from '../../../services/plans/list';
import { getPlayers } from '../../../services/players/list';
import { addTransaction } from '../../../services/transactions/add';
import { PaymentStatus, TransactionType } from '../../../services/transactions/list';

const formFields: FieldConfig<CreatePlanTransactionFormData>[] = [
  {
    name: 'playerId',
    label: 'Player',
    type: 'search',
    onSearch: async (query: string) => {
      const data = await getPlayers({
        limit: 100,
        username: query,
        isActive: 1,
        sortBy: 'username',
        sortOrder: 'ASC',
      });
      return data.data.map((player) => ({
        label: player.username,
        id: player.id,
      }));
    },
    required: true,
    placeholder: 'Start typing to search for players',
    formHelperText: 'Select the player who is making the transaction',
  },
  {
    name: 'planId',
    label: 'Plan',
    type: 'search',
    onSearch: async (query: string) => {
      const data = await getPlans({ limit: 100, isActive: 1, search: query });
      return data.data.map((plan) => ({
        label: `${plan.name} - ₹${plan.price} - ${plan.planType}`,
        id: plan.id,
      }));
    },
    required: true,
    placeholder: 'Start typing to search for plans',
    formHelperText: 'Select the plan to assign',
  },
  {
    name: 'paymentMethod',
    label: 'Payment Method',
    type: 'select',
    required: true,
    gridCols: 6,
    options: paymentMethodOptions,
    helperText: 'Select payment method',
    formHelperText:
      'Select the payment method for the transaction, Split Payment allows you to split the payment between cash and online',
  },
  {
    name: 'cashAmount',
    label: 'Cash Amount',
    type: 'number',
    placeholder: '0',
    gridCols: 6,
    helperText: 'Cash amount',
    formHelperText: 'Cash amount when payment method is Split Payment, else keep empty.',
  },
  {
    name: 'onlineAmount',
    label: 'Online Amount',
    type: 'number',
    placeholder: '0',
    gridCols: 6,
    helperText: 'Online amount',
    formHelperText: 'Online amount when payment method is Split Payment, else keep empty.',
  },
  {
    name: 'notes',
    label: 'Notes',
    type: 'textarea',
    placeholder: 'Add any additional notes...',
    fullWidth: true,
    rows: 3,
    helperText: 'Optional transaction notes (max 500 characters)',
  },
];

export default function AddNewPlanTransactionPage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  const handleSubmit = async (data: CreatePlanTransactionFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.playerId || !data.planId || !data.paymentMethod) {
      setError('Player, plan, and payment method are required');
      setLoading(false);
      return;
    }

    try {
      const payload = {
        playerId: data.playerId,
        transactionType: TransactionType.PLAN_PURCHASE,
        planId: data.planId,
        paymentMethod: data.paymentMethod as PaymentMethodType,
        paymentStatus: PaymentStatus.COMPLETED,
        notes: data.notes || undefined,
        cashAmount:
          data.paymentMethod === PaymentMethodValues.SPLIT_PAYMENT ? data.cashAmount : undefined,
        onlineAmount:
          data.paymentMethod === PaymentMethodValues.SPLIT_PAYMENT ? data.onlineAmount : undefined,
        transactionDate: data.transactionDate
          ? new Date(data.transactionDate).toISOString()
          : undefined,
      };

      await addTransaction(payload);

      setSuccess('Transaction created successfully!');

      // Navigate back to plan transactions list after a short delay
      setTimeout(() => {
        navigate('/plan-transactions');
      }, 1500);
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to create transaction');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/plan-transactions');
  };

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          New Plan Transaction
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Assign a plan to a player and create a transaction record
        </Typography>
      </Box>

      <FormBuilder<CreatePlanTransactionFormData>
        fields={formFields}
        schema={createPlanTransactionSchema}
        defaultValues={createPlanTransactionDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={loading}
        error={error}
        success={success}
        showCancel
        showReset
        submitLabel="Create Transaction"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
