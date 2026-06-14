import { permissionsForRole } from '@gaming-cafe/contracts';
import { CurrencyField, FormButton, FormTextField, IntegerField } from '@gaming-cafe/ui';
import { local, toastUtils, useAsyncAction } from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Card,
  CardActionArea,
  CardContent,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Grid,
  Step,
  StepLabel,
  Stepper,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useDispatch } from '../hooks/store';
import type { Permission } from '../hooks/usePermissions';
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
  const [expectedClosing, setExpectedClosing] = useState(0);
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

  useEffect(() => {
    if (!open) return;

    getExpectedClosing()
      .then((data) => {
        setExpectedClosing(data.expectedClosing);
      })
      .catch(() => {
        setExpectedClosing(0);
      });
  }, [open]);

  const closingAmount = useMemo(() => {
    const fromDenominations = sumDenominations(closingDenominations);
    if (fromDenominations > 0) return fromDenominations;
    return Number(closingBalance) || 0;
  }, [closingBalance, closingDenominations]);

  const depositAmount = useMemo(
    () => sumDenominations(depositDenominations),
    [depositDenominations],
  );

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

          local.remove('accessToken');
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
            validatorUsername,
            validatorPassword,
            validatorTotp,
            deposit: depositInput,
          });

          local.set('accessToken', response.newAccessToken);
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
            },
          });

          toastUtils.success('Shift handover completed');
          handleClose();
          void queryClient.invalidateQueries({ queryKey: ['activeShift'] });
          void queryClient.invalidateQueries({ queryKey: ['staffDashboardStats'] });
          void queryClient.invalidateQueries({ queryKey: ['shifts'] });
          void queryClient.invalidateQueries({ queryKey: ['cash-registers'] });
          const permissions = permissionsForRole(response.newUser.role);
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
            <Stepper activeStep={step} sx={{ mb: 1, mt: 1 }}>
              <Step>
                <StepLabel>Closing Balance</StepLabel>
              </Step>
              <Step>
                <StepLabel>Cash Deposit</StepLabel>
              </Step>
              {mode === 'handover' && (
                <Step>
                  <StepLabel>Validator</StepLabel>
                </Step>
              )}
            </Stepper>

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
        {mode && step < (mode === 'handover' ? 2 : 1) ? (
          <FormButton
            variant="contained"
            onClick={() => setStep((current) => current + 1)}
            disabled={loading || (step === 0 && closingAmount <= 0)}
          >
            Next
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
