import { FormButton, FormTextField, PasswordField } from '@gaming-cafe/ui';
import { local, toastUtils } from '@gaming-cafe/utils';
import { Box, ToggleButton, ToggleButtonGroup, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import OtpModal from '../../components/OtpModal';
import { setStaffShiftStart } from '../../constants/staffShift';
import { useDispatch } from '../../hooks/store';
import { loginAPI, loginStaffAPI, verifyOtpAPI } from '../../services/auth/auth';
import type { VerifyOtpResponseUser } from '../../services/auth/types';

type LoginMode = 'admin' | 'staff';

function isPanelRole(role: string): role is 'admin' | 'staff' {
  return role === 'admin' || role === 'staff';
}

export default function LoginPage() {
  const navigate = useNavigate();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [loginMode, setLoginMode] = useState<LoginMode>('admin');
  const [otpModalOpen, setOtpModalOpen] = useState(false);
  const [transactionId, setTransactionId] = useState('');
  const [otpLoading, setOtpLoading] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const dispatch = useDispatch();

  const completeLogin = (accessToken: string, user: VerifyOtpResponseUser) => {
    if (!isPanelRole(user.role)) {
      toastUtils.error('Forbidden Access: User is not authorized for this panel');
      return;
    }

    local.set('accessToken', accessToken);
    if (user.role === 'staff') {
      setStaffShiftStart(new Date().toISOString());
    }
    dispatch({
      type: 'SetAuthDetail',
      payload: {
        ...user,
      },
    });
    navigate('/');
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);

    try {
      if (loginMode === 'admin') {
        const response = await loginAPI(username, password);
        setTransactionId(response.transactionId);
        setOtpModalOpen(true);
        toastUtils.success('OTP sent successfully!');
      } else {
        const response = await loginStaffAPI(username, password);
        completeLogin(response.accessToken, response.user);
      }
    } catch (error: unknown) {
      const message =
        error instanceof Error ? error.message : 'Login failed. Please check your credentials.';
      toastUtils.error(message);
    } finally {
      setSubmitting(false);
    }
  };

  const handleVerifyOtp = async (otp: string) => {
    setOtpLoading(true);
    try {
      const response = await verifyOtpAPI(transactionId, otp);
      completeLogin(response.accessToken, response.user);
      setOtpModalOpen(false);
    } catch (_error) {
      toastUtils.error('OTP verification failed. Please try again.');
    } finally {
      setOtpLoading(false);
    }
  };

  return (
    <>
      <Box component="form" onSubmit={handleSubmit}>
        <Typography variant="h4" fontWeight={700} gutterBottom>
          Welcome back
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
          Enter your credentials to access your account
        </Typography>

        <ToggleButtonGroup
          value={loginMode}
          exclusive
          fullWidth
          onChange={(_event, value: LoginMode | null) => {
            if (value) {
              setLoginMode(value);
            }
          }}
          sx={{ mb: 3 }}
        >
          <ToggleButton value="admin">Admin</ToggleButton>
          <ToggleButton value="staff">Staff</ToggleButton>
        </ToggleButtonGroup>

        <FormTextField
          fullWidth
          variant="outlined"
          label="Username"
          type="text"
          value={username}
          onChange={(e) => setUsername(e.target.value)}
          sx={{ mb: 2.5 }}
          inputProps={{
            autoComplete: 'new-password',
            form: { autoComplete: 'off' },
          }}
        />

        <PasswordField
          fullWidth
          label="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
          sx={{ mb: 1.5 }}
          inputProps={{
            autoComplete: 'new-password',
            form: { autoComplete: 'off' },
          }}
        />

        <FormButton
          type="submit"
          variant="contained"
          fullWidth
          size="large"
          disabled={submitting}
          sx={{ py: 1.5, mb: 3, mt: 2 }}
        >
          {loginMode === 'admin' ? 'Sign in with OTP' : 'Sign in'}
        </FormButton>
      </Box>

      <OtpModal
        open={otpModalOpen}
        onClose={() => setOtpModalOpen(false)}
        onVerify={handleVerifyOtp}
        loading={otpLoading}
      />
    </>
  );
}
