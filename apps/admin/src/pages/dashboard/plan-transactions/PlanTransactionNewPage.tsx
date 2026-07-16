import type { PaymentStatusValue } from '@gaming-cafe/contracts';
import { FormButton } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import { Search as SearchIcon } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  CircularProgress,
  Divider,
  Grid,
  InputAdornment,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { ActiveShiftGuard } from '../../../components/ActiveShiftGuard';
import CreditEligibilityAlert from '../../../components/CreditEligibilityAlert';
import { PlanDeductionSummary } from '../../../containers/plans/PlanDeductionSummary';
import {
  CounterSaleLayout,
  evaluateCreditBlocked,
  PlanSaleCard,
  PosOnlinePaymentRefField,
  PosPaymentTiles,
  type PosPlayer,
  PosPlayerPicker,
  PosSplitAmountFields,
  posSaleSuccessLabel,
  requiresOnlinePaymentRef,
  validateOnlinePaymentRefLast4,
  validateSplitPaymentAmounts,
} from '../../../containers/sales';
import {
  type PaymentMethodType,
  PaymentMethodValues,
  PaymentStatusValues,
} from '../../../containers/transactions/schemas/transaction-schema';
import { getPlayerCredit } from '../../../services/credit';
import { getPlans, type PlanResponse } from '../../../services/plans/list';
import { getPlayerById } from '../../../services/players/getById';
import { addTransaction } from '../../../services/transactions/add';
import { PaymentStatus, TransactionType } from '../../../services/transactions/list';

export default function AddNewPlanTransactionPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const preselectedPlayerId = searchParams.get('playerId') ?? undefined;

  const {
    loading: submitting,
    succeeded,
    failed,
    errorMessage,
    disabled: submitDisabled,
    run,
    clearError,
  } = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });
  const [error, setError] = useState<string | undefined>();

  const [selectedPlayer, setSelectedPlayer] = useState<PosPlayer | null>(null);
  const [selectedPlan, setSelectedPlan] = useState<PlanResponse | null>(null);
  const [planSearch, setPlanSearch] = useState('');
  const [paymentMethod, setPaymentMethod] = useState<string>(PaymentMethodValues.CASH);
  const [cashAmount, setCashAmount] = useState<string>('');
  const [onlineAmount, setOnlineAmount] = useState<string>('');
  const [onlinePaymentRefLast4, setOnlinePaymentRefLast4] = useState<string>('');
  const [notes, setNotes] = useState<string>('');

  const { data: preselectedPlayer } = useQuery({
    queryKey: ['player', preselectedPlayerId],
    queryFn: () => getPlayerById(preselectedPlayerId as string),
    enabled: !!preselectedPlayerId,
  });

  useEffect(() => {
    if (preselectedPlayer) {
      setSelectedPlayer({ id: preselectedPlayer.id, username: preselectedPlayer.username });
    }
  }, [preselectedPlayer]);

  const checkoutErrorClearKey = [
    selectedPlayer?.id,
    selectedPlan?.id,
    paymentMethod,
    cashAmount,
    onlineAmount,
  ].join('|');

  // biome-ignore lint/correctness/useExhaustiveDependencies: clear API error when checkout inputs change after failure
  useEffect(() => {
    if (failed) clearError();
  }, [checkoutErrorClearKey, failed, clearError]);

  const { data: plansData, isLoading: plansLoading } = useQuery({
    queryKey: ['pos-plans', planSearch],
    queryFn: () =>
      getPlans({
        limit: 100,
        isActive: 1,
        search: planSearch.trim() || undefined,
      }),
  });

  const plans = useMemo(() => plansData?.data ?? [], [plansData]);

  const purchaseAmount = selectedPlan ? parseFloat(selectedPlan.price) : 0;
  const isCredit = paymentMethod === PaymentMethodValues.CREDIT;

  const { data: creditDetail, isFetching: creditLoading } = useQuery({
    queryKey: ['player-credit', selectedPlayer?.id],
    queryFn: () => getPlayerCredit(selectedPlayer?.id as string),
    enabled: isCredit && !!selectedPlayer?.id,
  });

  const creditBlocked = useMemo(
    () => evaluateCreditBlocked(isCredit, purchaseAmount, creditDetail, creditLoading),
    [isCredit, purchaseAmount, creditDetail, creditLoading],
  );

  const handleSubmit = () => {
    setError(undefined);

    if (!selectedPlayer) {
      setError('Please select a player');
      return;
    }

    if (!selectedPlan) {
      setError('Please select a plan');
      return;
    }

    if (paymentMethod === PaymentMethodValues.SPLIT_PAYMENT) {
      const splitError = validateSplitPaymentAmounts(purchaseAmount, cashAmount, onlineAmount);
      if (splitError) {
        setError(splitError);
        return;
      }
    }

    const onlinePortion =
      paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
        ? Number.parseFloat(onlineAmount)
        : paymentMethod === PaymentMethodValues.ONLINE
          ? purchaseAmount
          : undefined;
    const refError = validateOnlinePaymentRefLast4(
      onlinePaymentRefLast4,
      paymentMethod,
      onlinePortion,
    );
    if (refError) {
      setError(refError);
      return;
    }

    if (isCredit && creditBlocked) {
      setError('This player is not eligible for this credit purchase');
      return;
    }

    void run(async () => {
      const payload = {
        playerId: selectedPlayer.id,
        transactionType: TransactionType.PLAN_PURCHASE,
        planId: selectedPlan.id,
        paymentMethod: paymentMethod as PaymentMethodType,
        paymentStatus:
          paymentMethod === PaymentMethodValues.CREDIT
            ? PaymentStatusValues.CREDIT
            : PaymentStatus.COMPLETED,
        notes: notes || undefined,
        cashAmount:
          paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
            ? parseFloat(cashAmount)
            : paymentMethod === PaymentMethodValues.CASH
              ? purchaseAmount
              : undefined,
        onlineAmount:
          paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
            ? parseFloat(onlineAmount)
            : paymentMethod === PaymentMethodValues.ONLINE
              ? purchaseAmount
              : undefined,
        onlinePaymentRefLast4: requiresOnlinePaymentRef(paymentMethod, onlinePortion)
          ? onlinePaymentRefLast4.trim()
          : undefined,
      };

      try {
        await addTransaction({
          ...payload,
          paymentStatus: payload.paymentStatus as PaymentStatusValue,
        });

        setTimeout(() => {
          navigate('/plan-transactions');
        }, 1500);
      } catch (err: unknown) {
        throw err instanceof Error ? err : new Error('Failed to create transaction');
      }
    });
  };

  const handleCancel = () => {
    navigate('/plan-transactions');
  };

  const alerts = (
    <>
      {preselectedPlayer && (
        <Alert severity="info" sx={{ mb: 2 }}>
          Buying plan for <strong>{preselectedPlayer.username}</strong>
        </Alert>
      )}
      {error && (
        <Alert severity="error" sx={{ mb: 3 }} onClose={() => setError(undefined)}>
          {error}
        </Alert>
      )}
    </>
  );

  const catalog = (
    <>
      <PosPlayerPicker value={selectedPlayer} onChange={setSelectedPlayer} disabled={submitting} />

      <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
        Plans
      </Typography>
      <TextField
        placeholder="Search plans..."
        value={planSearch}
        onChange={(e) => setPlanSearch(e.target.value)}
        fullWidth
        size="small"
        disabled={submitting}
        sx={{ mb: 2 }}
        helperText="Filter active plans by name or type"
        InputProps={{
          startAdornment: (
            <InputAdornment position="start">
              <SearchIcon fontSize="small" color="action" />
            </InputAdornment>
          ),
        }}
      />

      {plansLoading ? (
        <Box sx={{ display: 'flex', justifyContent: 'center', py: 6 }}>
          <CircularProgress size={32} />
        </Box>
      ) : plans.length === 0 ? (
        <Typography variant="body2" color="text.secondary" sx={{ py: 4, textAlign: 'center' }}>
          {planSearch ? 'No plans match your search' : 'No active plans available'}
        </Typography>
      ) : (
        <Grid container spacing={2}>
          {plans.map((plan) => (
            <Grid key={plan.id} size={{ xs: 12, sm: 6, md: 4 }}>
              <PlanSaleCard
                plan={plan}
                selected={selectedPlan?.id === plan.id}
                onSelect={setSelectedPlan}
                disabled={submitting}
              />
            </Grid>
          ))}
        </Grid>
      )}
    </>
  );

  const summary = (
    <>
      <Card variant="outlined" sx={{ mb: 3 }}>
        <CardContent>
          <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
            Selected plan
          </Typography>

          {!selectedPlan ? (
            <Typography variant="body2" color="text.secondary" sx={{ py: 4, textAlign: 'center' }}>
              Select a plan from the grid
            </Typography>
          ) : (
            <>
              <Typography variant="body1" fontWeight={600}>
                {selectedPlan.name}
              </Typography>
              <Typography variant="caption" color="text.secondary" display="block">
                {selectedPlan.planType}
              </Typography>
              {selectedPlan.timeCredits != null && (
                <Typography variant="body2" sx={{ mt: 1 }}>
                  {selectedPlan.timeCredits} wallet minutes credited
                </Typography>
              )}
              <Typography variant="body2" color="text.secondary">
                Valid {selectedPlan.validityDays} days
              </Typography>

              {selectedPlan.dynamicDeductionEnabled && selectedPlan.deductionProfile && (
                <PlanDeductionSummary
                  timeCredits={selectedPlan.timeCredits ?? 0}
                  deductionProfile={selectedPlan.deductionProfile}
                />
              )}

              <Divider sx={{ my: 2 }} />

              <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
                <Typography variant="h6">Total</Typography>
                <Typography variant="h6" fontWeight={700}>
                  ₹{purchaseAmount.toFixed(2)}
                </Typography>
              </Box>
            </>
          )}
        </CardContent>
      </Card>

      <Card variant="outlined">
        <CardContent>
          <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
            Payment
          </Typography>

          <PosPaymentTiles
            value={paymentMethod}
            onChange={setPaymentMethod}
            disabled={submitting}
          />

          <CreditEligibilityAlert
            playerId={selectedPlayer?.id}
            paymentMethod={paymentMethod}
            purchaseAmount={purchaseAmount}
          />

          {paymentMethod === PaymentMethodValues.SPLIT_PAYMENT && (
            <PosSplitAmountFields
              cashAmount={cashAmount}
              onlineAmount={onlineAmount}
              onCashChange={setCashAmount}
              onOnlineChange={setOnlineAmount}
              totalAmount={purchaseAmount}
            />
          )}

          {requiresOnlinePaymentRef(
            paymentMethod,
            paymentMethod === PaymentMethodValues.SPLIT_PAYMENT ? onlineAmount : purchaseAmount,
          ) && (
            <PosOnlinePaymentRefField
              value={onlinePaymentRefLast4}
              onChange={setOnlinePaymentRefLast4}
              disabled={submitting}
            />
          )}

          <TextField
            label="Notes (Optional)"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            fullWidth
            multiline
            rows={2}
            disabled={submitting}
            placeholder="Add any notes about this transaction..."
            helperText="Optional staff note stored on the transaction (max 500 chars)"
            sx={{ mb: 3 }}
          />

          <Stack spacing={2}>
            <FormButton
              variant="contained"
              size="large"
              fullWidth
              onClick={handleSubmit}
              loading={submitting}
              success={succeeded}
              successLabel={posSaleSuccessLabel(paymentMethod)}
              error={failed}
              errorLabel={errorMessage ?? 'Failed to create transaction'}
              disabled={!selectedPlayer || !selectedPlan || creditBlocked || submitDisabled}
              sx={{ minHeight: 44 }}
            >
              Complete sale
            </FormButton>
            <Button
              variant="outlined"
              size="large"
              fullWidth
              onClick={handleCancel}
              disabled={submitting}
              sx={{ minHeight: 44 }}
            >
              Cancel
            </Button>
          </Stack>
        </CardContent>
      </Card>
    </>
  );

  return (
    <ActiveShiftGuard>
      <CounterSaleLayout
        backTo="/plan-transactions"
        backLabel="Plan sales"
        alerts={alerts}
        catalog={catalog}
        summary={summary}
      />
    </ActiveShiftGuard>
  );
}
