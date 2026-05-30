import type { PaymentStatusValue } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import CreditEligibilityAlert from '../../../components/CreditEligibilityAlert';
import {
  type CreatePlanTransactionFormData,
  createPlanTransactionDefaultValues,
  createPlanTransactionSchema,
  type PaymentMethodType,
  PaymentMethodValues,
  PaymentStatusValues,
  paymentMethodOptions,
} from '../../../containers/transactions/schemas/transaction-schema';
import { getPlayerCredit } from '../../../services/credit';
import { getPlanById } from '../../../services/plans/getById';
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
  const [formValues, setFormValues] = useState<Partial<CreatePlanTransactionFormData>>(
    createPlanTransactionDefaultValues,
  );

  const { data: selectedPlan } = useQuery({
    queryKey: ['plan', formValues.planId],
    queryFn: () => getPlanById(formValues.planId as string),
    enabled: !!formValues.planId,
  });

  const purchaseAmount = selectedPlan?.price ?? 0;
  const isCredit = formValues.paymentMethod === PaymentMethodValues.CREDIT;

  const { data: creditDetail } = useQuery({
    queryKey: ['player-credit', formValues.playerId],
    queryFn: () => getPlayerCredit(formValues.playerId as string),
    enabled: isCredit && !!formValues.playerId,
  });

  const creditBlocked = useMemo(() => {
    if (!isCredit || !creditDetail?.summary) return false;
    const s = creditDetail.summary;
    if (!s.creditEnabled || s.creditLimit <= 0) return true;
    return Number(purchaseAmount) > Number(s.available) + 0.001;
  }, [isCredit, creditDetail, purchaseAmount]);

  const handleSubmit = async (data: CreatePlanTransactionFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.playerId || !data.planId || !data.paymentMethod) {
      setError('Player, plan, and payment method are required');
      setLoading(false);
      return;
    }

    if (data.paymentMethod === PaymentMethodValues.CREDIT && creditBlocked) {
      setError('This player is not eligible for this credit purchase');
      setLoading(false);
      return;
    }

    try {
      const payload = {
        playerId: data.playerId,
        transactionType: TransactionType.PLAN_PURCHASE,
        planId: data.planId,
        paymentMethod: data.paymentMethod as PaymentMethodType,
        paymentStatus:
          data.paymentMethod === PaymentMethodValues.CREDIT
            ? PaymentStatusValues.CREDIT
            : PaymentStatus.COMPLETED,
        notes: data.notes || undefined,
        cashAmount:
          data.paymentMethod === PaymentMethodValues.SPLIT_PAYMENT ? data.cashAmount : undefined,
        onlineAmount:
          data.paymentMethod === PaymentMethodValues.SPLIT_PAYMENT ? data.onlineAmount : undefined,
        transactionDate: data.transactionDate
          ? new Date(data.transactionDate).toISOString()
          : undefined,
      };

      await addTransaction({
        ...payload,
        paymentStatus: payload.paymentStatus as PaymentStatusValue,
      });

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

      <CreditEligibilityAlert
        playerId={formValues.playerId}
        paymentMethod={formValues.paymentMethod}
        purchaseAmount={Number(purchaseAmount)}
      />

      <FormBuilder<CreatePlanTransactionFormData>
        fields={formFields}
        schema={createPlanTransactionSchema}
        defaultValues={createPlanTransactionDefaultValues}
        onChange={(values) => setFormValues(values)}
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
