import { FormButton, FormTextField } from '@gaming-cafe/ui';
import { local, toastUtils } from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  GridLegacy as Grid,
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
import { DENOMINATIONS } from '../services/cash-registers';
import { closeShift, getExpectedClosing, handoverShift } from '../services/shifts';

interface ShiftHandoverDialogProps {
  open: boolean;
  onClose: () => void;
}

function sumDenominations(denominations: Record<string, number>) {
  return Object.entries(denominations).reduce(
    (total, [value, count]) => total + Number(value) * count,
    0,
  );
}

export default function ShiftHandoverDialog({ open, onClose }: ShiftHandoverDialogProps) {
  const navigate = useNavigate();
  const dispatch = useDispatch();
  const queryClient = useQueryClient();
  const [step, setStep] = useState(0);
  const [loading, setLoading] = useState(false);
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
        <Grid item xs={6} sm={4} md={3} key={value}>
          <TextField
            fullWidth
            size="small"
            type="number"
            label={`₹${value}`}
            value={denominations[String(value)] ?? ''}
            onChange={(event) => handleDenominationChange(setter, value, event.target.value)}
            inputProps={{ min: 0 }}
          />
        </Grid>
      ))}
    </Grid>
  );

  const completeHandover = async () => {
    setLoading(true);
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
        navigate('/');
      }
    } catch (error: unknown) {
      const message =
        error instanceof Error
          ? error.message
          : `Shift ${mode === 'close' ? 'close' : 'handover'} failed`;
      toastUtils.error(message);
    } finally {
      setLoading(false);
    }
  };

  return (
    <Dialog open={open} onClose={handleClose} fullWidth maxWidth="md">
      <DialogTitle>End Shift</DialogTitle>
      <DialogContent>
        {!mode && (
          <Box sx={{ display: 'flex', flexDirection: 'column', gap: 2, pt: 2 }}>
            <Button variant="outlined" size="large" onClick={() => setMode('handover')}>
              Handover to next staff
            </Button>
            <Button variant="outlined" size="large" onClick={() => setMode('close')}>
              Close shift (no replacement)
            </Button>
          </Box>
        )}

        {mode && (
          <>
            <Stepper activeStep={step} sx={{ mb: 3, mt: 2 }}>
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

            {mode && step === 0 && (
              <Box>
                <Alert severity="info" sx={{ mb: 2 }}>
                  Expected closing balance: ₹{expectedClosing.toFixed(2)}
                </Alert>
                <FormTextField
                  fullWidth
                  label="Closing Balance (optional if using denominations)"
                  type="number"
                  value={closingBalance}
                  onChange={(event) => setClosingBalance(event.target.value)}
                  sx={{ mb: 2 }}
                />
                <Typography variant="subtitle2">Closing Denominations</Typography>
                {renderDenominationGrid(closingDenominations, setClosingDenominations)}
                <Typography variant="body2" sx={{ mt: 2 }}>
                  Counted total: ₹{closingAmount.toFixed(2)}
                </Typography>
                {variance !== 0 && (
                  <Alert
                    severity={Math.abs(variance) > 0.01 ? 'warning' : 'success'}
                    sx={{ mt: 2 }}
                  >
                    Variance: ₹{variance.toFixed(2)}
                  </Alert>
                )}
                <TextField
                  fullWidth
                  label="Notes"
                  value={notes}
                  onChange={(event) => setNotes(event.target.value)}
                  sx={{ mt: 2 }}
                  multiline
                  minRows={2}
                />
              </Box>
            )}

            {mode && step === 1 && (
              <Box>
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
                  <Alert severity="info" sx={{ mt: 2 }}>
                    No deposit will be created. You can choose deposit on any shift close.
                  </Alert>
                )}
                {includeDeposit && (
                  <Box sx={{ mt: 2 }}>
                    <Typography variant="subtitle2">Deposit Denominations</Typography>
                    {renderDenominationGrid(depositDenominations, setDepositDenominations)}
                    <Typography variant="body2" sx={{ mt: 2 }}>
                      Deposit amount: ₹{depositAmount.toFixed(2)}
                    </Typography>
                    <TextField
                      fullWidth
                      label="Deposit Notes"
                      value={depositNotes}
                      onChange={(event) => setDepositNotes(event.target.value)}
                      sx={{ mt: 2 }}
                      multiline
                      minRows={2}
                    />
                    <Alert severity="warning" sx={{ mt: 2 }}>
                      Admin must physically verify denominations and approve the deposit destination
                      (bank or home).
                    </Alert>
                  </Box>
                )}
              </Box>
            )}

            {mode === 'handover' && step === 2 && (
              <Box>
                <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
                  Another staff member must validate the closing balance and enter their TOTP code.
                </Typography>
                <FormTextField
                  fullWidth
                  label="Validator Username"
                  value={validatorUsername}
                  onChange={(event) => setValidatorUsername(event.target.value)}
                  sx={{ mb: 2 }}
                />
                <FormTextField
                  fullWidth
                  label="Validator Password"
                  type="password"
                  value={validatorPassword}
                  onChange={(event) => setValidatorPassword(event.target.value)}
                  sx={{ mb: 2 }}
                />
                <FormTextField
                  fullWidth
                  label="Validator TOTP Code"
                  value={validatorTotp}
                  onChange={(event) =>
                    setValidatorTotp(event.target.value.replace(/\s+/g, '').slice(0, 6))
                  }
                  inputProps={{ autoComplete: 'one-time-code' }}
                />
              </Box>
            )}
          </>
        )}
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <FormButton onClick={handleClose} disabled={loading}>
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
            disabled={
              loading ||
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
