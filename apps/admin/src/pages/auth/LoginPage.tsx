import { FormButton, FormTextField, PasswordField } from '@gaming-cafe/ui';
import { local, toastUtils } from '@gaming-cafe/utils';
import { Box, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import OtpModal from '../../components/OtpModal';
import { useDispatch } from '../../hooks/store';
import { loginAPI, verifyOtpAPI } from '../../services/auth/auth';

export default function LoginPage() {
  const navigate = useNavigate();
  const [email, setEmail] = useState('');
  const [password, setPassword] = useState('');
  const [otpModalOpen, setOtpModalOpen] = useState(false);
  const [transactionId, setTransactionId] = useState('');
  const [otpLoading, setOtpLoading] = useState(false);
  const dispatch = useDispatch();

  const handleSubmit = async (e: React.FormEvent) => {
    try {
      e.preventDefault();
      const response = await loginAPI(email, password);
      setTransactionId(response.transactionId);
      setOtpModalOpen(true);
      toastUtils.success('OTP sent successfully!');
    } catch (error: unknown) {
      const message =
        error instanceof Error ? error.message : 'Login failed. Please check your credentials.';
      toastUtils.error(message);
    }
  };

  const handleVerifyOtp = async (otp: string) => {
    setOtpLoading(true);
    try {
      const response = await verifyOtpAPI(transactionId, otp);
      if (response?.user.role === 'admin') {
        local.set('accessToken', response?.accessToken);
        setOtpModalOpen(false);
        dispatch({
          type: 'SetAuthDetail',
          payload: {
            ...response.user,
          },
        });
        navigate('/');
      } else {
        toastUtils.error('Forbidden Access: User is not an admin');
      }
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
        <Typography variant="body2" color="text.secondary" sx={{ mb: 4 }}>
          Enter your credentials to access your account
        </Typography>

        <FormTextField
          fullWidth
          variant="outlined"
          label="Username"
          type="text"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
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
          sx={{ py: 1.5, mb: 3, mt: 2 }}
        >
          Sign in
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
