import {
  CurrencyField,
  FormButton,
  FormTextField,
  IntegerField,
  WizardProgress,
} from '@gaming-cafe/ui';
import {
  local,
  normalizeUsername,
  toastUtils,
  trimValue,
  useAsyncAction,
} from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Button,
  Card,
  CardActionArea,
  CardContent,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Grid,
  LinearProgress,
  Stack,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useDispatch } from '../hooks/store';
import type { Permission } from '../hooks/usePermissions';
import { clearAdminSession, sessionPermissions, setAdminToken } from '../lib/authSession';
import { DENOMINATIONS } from '../services/cash-registers';
import { closeShift, getExpectedClosing, handoverShift } from '../services/shifts';
import { getDefaultHomePath } from '../utils/homePath';

interface ShiftHandoverDialogProps {
  open: boolean;
  onClose: () => void;
}

const MODE_OPTIONS = [
  {
    mode: 'handover' as const,
    title: 'Handover to next staff',
    description: 'Validator confirms balance and session continues',
  },
  {
    mode: 'close' as const,
    title: 'Close shift (no replacement)',
    description: 'Ends shift and logs you out',
  },
];

function sumDenominations(denominations: Record<string, number>) {
  return Object.entries(denominations).reduce(
    (total, [value, count]) => total + Number(value) * count,
    0,
  );
}

function varianceColor(variance: number): string {
  if (Math.abs(variance) <= 0.01) return 'success.main';
  return 'warning.main';
}

export default function ShiftHandoverDialog({ open, onClose }: ShiftHandoverDialogProps) {
  const navigate = useNavigate();
  const dispatch = useDispatch();
  const queryClient = useQueryClient();
  const [step, setStep] = useState(0);
  const { loading, succeeded, failed, errorMessage, disabled, run, reset } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });

  const [closingBalance, setClosingBalance] = useState('');
  const [closingDenominations, setClosingDenominations] = useState<Record<string, number>>({});
  const [notes, setNotes] = useState('');
  const [includeDeposit, setIncludeDeposit] = useState(false);
  const [depositDenominations, setDepositDenominations] = useState<Record<string, number>>({});
  const [depositNotes, setDepositNotes] = useState('');
  const [validatorUsername, setValidatorUsername] = useState('');
  const [validatorPassword, setValidatorPassword] = useState('');
  const [validatorTotp, setValidatorTotp] = useState('');
  const [mode, setMode] = useState<'handover' | 'close' | null>(null);

  const closingQuery = useQuery({
    queryKey: ['expected-closing'],
    queryFn: getExpectedClosing,
    enabled: open,
    staleTime: 0,
    retry: false,
  });
  const expectedClosing = closingQuery.data?.expectedClosing ?? 0;

  const closingAmount = useMemo(() => {
    const fromDenominations = sumDenominations(closingDenominations);
    if (fromDenominations > 0) return fromDenominations;
    return Number(closingBalance);
  }, [closingBalance, closingDenominations]);

  const depositAmount = useMemo(
    () => sumDenominations(depositDenominations),
    [depositDenominations],
  );

  const closingConfirmed =
    closingBalance.trim() !== '' || sumDenominations(closingDenominations) > 0;
  const closingInvalid = !closingConfirmed || !Number.isFinite(closingAmount) || closingAmount < 0;
  const variance = closingAmount - expectedClosing;

  const resetState = () => {
    setStep(0);
    setMode(null);
    setClosingBalance('');
    setClosingDenominations({});
    setNotes('');
    setIncludeDeposit(false);
    setDepositDenominations({});
    setDepositNotes('');
    setValidatorUsername('');
    setValidatorPassword('');
    setValidatorTotp('');
    reset();
  };

  const handleClose = () => {
    resetState();
    onClose();
  };

  const handleDenominationChange = (
    setter: React.Dispatch<React.SetStateAction<Record<string, number>>>,
    value: number,
    count: string,
  ) => {
    const parsed = Number.parseInt(count, 10);
    setter((current) => ({
      ...current,
      [String(value)]: Number.isNaN(parsed) || parsed < 0 ? 0 : parsed,
    }));
  };

  const renderDenominationGrid = (
    denominations: Record<string, number>,
    setter: React.Dispatch<React.SetStateAction<Record<string, number>>>,
  ) => (
    <Grid container spacing={1.5} sx={{ mt: 1 }}>
      {DENOMINATIONS.notes.map((value) => (
        <Grid key={value} size={{ xs: 6, sm: 4, md: 3 }}>
          <IntegerField
            fullWidth
            size="small"
            label={`₹${value}`}
            value={denominations[String(value)] ?? ''}
            onChange={(event) => handleDenominationChange(setter, value, event.target.value)}
          />
        </Grid>
      ))}
    </Grid>
  );

  const completeHandover = () => {
    if (closingInvalid || closingQuery.isError || closingQuery.isLoading) return;
    const requestToken = local.get('accessToken');
    void run(async () => {
      try {
        const depositInput =
          includeDeposit && depositAmount > 0
            ? {
                amount: depositAmount,
                denominations: depositDenominations,
                notes: depositNotes || undefined,
              }
            : undefined;

        if (mode === 'close') {
          await closeShift({
            closingBalance: closingAmount,
            closingDenominations:
              Object.keys(closingDenominations).length > 0 ? closingDenominations : undefined,
            notes: notes || undefined,
            deposit: depositInput,
          });

          if (local.get('accessToken') !== requestToken) return;
          clearAdminSession();
          dispatch({ type: 'Reset' });
          toastUtils.success('Shift closed successfully');
          handleClose();
          navigate('/login');
        } else {
          const response = await handoverShift({
            closingBalance: closingAmount,
            closingDenominations:
              Object.keys(closingDenominations).length > 0 ? closingDenominations : undefined,
            notes: notes || undefined,
            validatorUsername: normalizeUsername(validatorUsername),
            validatorPassword: trimValue(validatorPassword),
            validatorTotp: trimValue(validatorTotp),
            deposit: depositInput,
          });

          if (local.get('accessToken') !== requestToken) return;
          setAdminToken(response.newAccessToken);
          dispatch({
            type: 'SetAuthDetail',
            payload: {
              id: response.newUser.id,
              email: response.newUser.email ?? '',
              username: response.newUser.username,
              firstName: response.newUser.firstName ?? '',
              lastName: response.newUser.lastName ?? '',
              role: response.newUser.role,
              isActive: response.newUser.isActive,
              avatarUrl: response.newUser.avatarUrl ?? '',
            },
          });

          toastUtils.success('Shift handover completed');
          handleClose();
          void queryClient.invalidateQueries({ queryKey: ['activeShift'] });
          void queryClient.invalidateQueries({ queryKey: ['staffDashboardStats'] });
          void queryClient.invalidateQueries({ queryKey: ['shifts'] });
          void queryClient.invalidateQueries({ queryKey: ['cash-registers'] });
          const permissions = sessionPermissions();
          const can = (permission: Permission) => permissions.includes(permission);
          navigate(getDefaultHomePath(can));
        }
      } catch (error: unknown) {
        const message =
          error instanceof Error
            ? error.message
            : `Shift ${mode === 'close' ? 'close' : 'handover'} failed`;
        toastUtils.error(message);
        throw error instanceof Error ? error : new Error(message);
      }
    });
  };

  return (
    <Dialog
      open={open}
      onClose={loading ? undefined : handleClose}
      fullWidth
      maxWidth="md"
      PaperProps={{ sx: { borderRadius: 2 } }}
    >
      <DialogTitle sx={{ pb: 1 }}>
        <Typography variant="subtitle1" component="div" fontWeight={600}>
          End shift
        </Typography>
      </DialogTitle>
      <DialogContent sx={{ pt: 1 }}>
        {closingQuery.isLoading && <LinearProgress aria-label="Loading drawer balance" />}
        {closingQuery.isError && (
          <Alert
            severity="error"
            action={<Button onClick={() => void closingQuery.refetch()}>Retry</Button>}
          >
            Could not verify the drawer balance. Retry before closing the shift.
          </Alert>
        )}
        {!mode && (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: 1 }}>
            {MODE_OPTIONS.map((option) => (
              <Card key={option.mode} variant="outlined">
                <CardActionArea onClick={() => setMode(option.mode)} sx={{ minHeight: 56 }}>
                  <CardContent sx={{ py: 2, '&:last-child': { pb: 2 } }}>
                    <Typography variant="body1" fontWeight={600}>
                      {option.title}
                    </Typography>
                    <Typography variant="body2" color="text.secondary">
                      {option.description}
                    </Typography>
                  </CardContent>
                </CardActionArea>
              </Card>
            ))}
          </Box>
        )}

        {mode && (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            <WizardProgress
              titles={[
                'Closing balance',
                'Cash deposit',
                ...(mode === 'handover' ? ['Replacement staff'] : []),
                'Review & confirm',
              ]}
              activeStep={step}
              disabled={loading}
              onBackTo={setStep}
            />
            {step === (mode === 'handover' ? 3 : 2) && (
              <Stack spacing={2}>
                <Alert severity="info">
                  Confirm the counted cash and deposit before closing this shift.
                </Alert>
                <Typography>Expected cash: ₹{expectedClosing.toFixed(2)}</Typography>
                <Typography>Counted cash: ₹{closingAmount.toFixed(2)}</Typography>
                <Typography color={variance ? 'warning.main' : 'text.primary'}>
                  Variance: ₹{variance.toFixed(2)}
                </Typography>
                <Typography>
                  Deposit: ₹{includeDeposit ? depositAmount.toFixed(2) : '0.00'}
                </Typography>
                {mode === 'handover' && (
                  <Typography>Replacement staff: {validatorUsername}</Typography>
                )}
                {notes && <Typography>Shift note: {notes}</Typography>}
                {includeDeposit && depositNotes && (
                  <Typography>Deposit note: {depositNotes}</Typography>
                )}
              </Stack>
            )}

            {step === 0 && (
              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <Card variant="outlined">
                  <CardContent>
                    <Grid container spacing={2}>
                      <Grid size={{ xs: 4 }}>
                        <Typography variant="caption" color="text.secondary" display="block">
                          Expected
                        </Typography>
                        <Typography variant="body1" fontWeight={600}>
                          ₹{expectedClosing.toFixed(2)}
                        </Typography>
                      </Grid>
                      <Grid size={{ xs: 4 }}>
                        <Typography variant="caption" color="text.secondary" display="block">
                          Counted
                        </Typography>
                        <Typography variant="body1" fontWeight={600}>
                          ₹{closingAmount.toFixed(2)}
                        </Typography>
                      </Grid>
                      <Grid size={{ xs: 4 }}>
                        <Typography variant="caption" color="text.secondary" display="block">
                          Variance
                        </Typography>
                        <Typography
                          variant="body1"
                          fontWeight={600}
                          color={varianceColor(variance)}
                        >
                          ₹{variance.toFixed(2)}
                        </Typography>
                      </Grid>
                    </Grid>
                  </CardContent>
                </Card>

                {Math.abs(variance) > 0.01 && (
                  <Alert severity="warning">
                    Counted balance differs from expected by ₹{Math.abs(variance).toFixed(2)}.
                  </Alert>
                )}

                <CurrencyField
                  fullWidth
                  size="small"
                  label="Closing Balance (optional if using denominations)"
                  value={closingBalance}
                  onChange={(event) => setClosingBalance(event.target.value)}
                />

                <Box>
                  <Typography variant="subtitle1" fontWeight={600} gutterBottom>
                    Closing denominations
                  </Typography>
                  {renderDenominationGrid(closingDenominations, setClosingDenominations)}
                </Box>

                <TextField
                  fullWidth
                  size="small"
                  label="Notes"
                  value={notes}
                  onChange={(event) => setNotes(event.target.value)}
                  multiline
                  minRows={2}
                />
              </Box>
            )}

            {step === 1 && (
              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <FormControlLabel
                  control={
                    <Switch
                      checked={includeDeposit}
                      onChange={(event) => setIncludeDeposit(event.target.checked)}
                    />
                  }
                  label="Withdraw cash for deposit today"
                />
                {!includeDeposit && (
                  <Alert severity="info">
                    No deposit will be created. You can choose deposit on any shift close.
                  </Alert>
                )}
                {includeDeposit && (
                  <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                    <Box>
                      <Typography variant="subtitle1" fontWeight={600} gutterBottom>
                        Deposit denominations
                      </Typography>
                      {renderDenominationGrid(depositDenominations, setDepositDenominations)}
                      <Typography variant="body2" sx={{ mt: 2 }}>
                        Deposit amount: ₹{depositAmount.toFixed(2)}
                      </Typography>
                    </Box>
                    <TextField
                      fullWidth
                      size="small"
                      label="Deposit Notes"
                      value={depositNotes}
                      onChange={(event) => setDepositNotes(event.target.value)}
                      multiline
                      minRows={2}
                    />
                    <Alert severity="warning">
                      Admin must physically verify denominations and approve the deposit destination
                      (bank or home).
                    </Alert>
                  </Box>
                )}
              </Box>
            )}

            {mode === 'handover' && step === 2 && (
              <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                <Typography variant="body2" color="text.secondary">
                  Another staff member must validate the closing balance and enter their TOTP code.
                </Typography>
                <FormTextField
                  fullWidth
                  size="small"
                  label="Validator username"
                  value={validatorUsername}
                  onChange={(event) => setValidatorUsername(event.target.value)}
                />
                <FormTextField
                  fullWidth
                  size="small"
                  label="Validator password"
                  type="password"
                  value={validatorPassword}
                  onChange={(event) => setValidatorPassword(event.target.value)}
                />
                <FormTextField
                  fullWidth
                  size="small"
                  label="Authenticator code"
                  placeholder="6-digit code"
                  helperText="Enter the code from the validator's authenticator app"
                  value={validatorTotp}
                  onChange={(event) =>
                    setValidatorTotp(event.target.value.replace(/\s+/g, '').slice(0, 6))
                  }
                  inputProps={{ autoComplete: 'one-time-code', inputMode: 'numeric' }}
                />
              </Box>
            )}
          </Box>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <FormButton variant="text" onClick={handleClose} disabled={loading}>
          Cancel
        </FormButton>
        {mode && step > 0 && (
          <FormButton onClick={() => setStep((current) => current - 1)} disabled={loading}>
            Back
          </FormButton>
        )}
        {mode && step < (mode === 'handover' ? 3 : 2) ? (
          <FormButton
            variant="contained"
            onClick={() => setStep((current) => current + 1)}
            disabled={
              loading ||
              closingQuery.isLoading ||
              closingQuery.isError ||
              (step === 0 && closingInvalid) ||
              (step === 1 && includeDeposit && depositAmount <= 0) ||
              (step === 2 &&
                mode === 'handover' &&
                (!validatorUsername || !validatorPassword || validatorTotp.length !== 6))
            }
          >
            {step === (mode === 'handover' ? 2 : 1) ? 'Review details' : 'Next'}
          </FormButton>
        ) : mode ? (
          <FormButton
            variant="contained"
            onClick={completeHandover}
            loading={loading}
            success={succeeded}
            successLabel={mode === 'handover' ? 'Handover complete' : 'Shift closed'}
            error={failed}
            errorLabel={errorMessage ?? 'Request failed'}
            disabled={
              disabled ||
              closingInvalid ||
              closingQuery.isLoading ||
              closingQuery.isError ||
              (includeDeposit && depositAmount <= 0) ||
              (mode === 'handover' &&
                (!validatorUsername || !validatorPassword || validatorTotp.length !== 6))
            }
          >
            {mode === 'handover' ? 'Complete Handover' : 'Close Shift'}
          </FormButton>
        ) : null}
      </DialogActions>
    </Dialog>
  );
}
