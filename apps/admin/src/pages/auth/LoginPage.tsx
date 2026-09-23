import { permissionsForRole } from '@gaming-cafe/contracts';
import { FormButton, OtpField, PasswordField, UsernameField } from '@gaming-cafe/ui';
import { isApiError, local, normalizeUsername, toastUtils, trimValue } from '@gaming-cafe/utils';
import { Alert, Box, Typography } from '@mui/material';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useDispatch } from '../../hooks/store';
import type { Permission } from '../../hooks/usePermissions';
import { loginPanelAPI, verifyPanelMfaAPI } from '../../services/auth/auth';
import type { PanelLoginResponse, VerifyOtpResponseUser } from '../../services/auth/types';
import { getDefaultHomePath } from '../../utils/homePath';

function isPanelRole(role: string): role is 'admin' | 'staff' {
  return role === 'admin' || role === 'staff';
}

function loginErrorMessage(error: unknown): string {
  if (isApiError(error)) {
    if (error.message === 'AUTH_INVALID_CREDENTIALS') return 'Username or password is incorrect.';
    if (error.message === 'AUTH_INVALID_MFA') return 'That authenticator code is not valid.';
    if (error.message === 'AUTH_CHALLENGE_EXPIRED') {
      return 'Your verification session expired. Sign in again.';
    }
  }
  return error instanceof Error ? error.message : 'Unable to sign in. Please try again.';
}

export default function LoginPage() {
  const navigate = useNavigate();
  const dispatch = useDispatch();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [totp, setTotp] = useState('');
  const [challengeToken, setChallengeToken] = useState<string | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!local.get('accessToken')) dispatch({ type: 'Reset' });
  }, [dispatch]);

  const completeLogin = (response: Extract<PanelLoginResponse, { status: 'authenticated' }>) => {
    const user: VerifyOtpResponseUser = response.user;
    if (!isPanelRole(user.role)) {
      setError('This account cannot access the operations panel.');
      return;
    }
    local.set('accessToken', response.accessToken);
    dispatch({
      type: 'SetAuthDetail',
      payload: {
        id: user.id,
        email: user.email ?? '',
        username: user.username,
        firstName: user.firstName ?? '',
        lastName: user.lastName ?? '',
        role: user.role,
        isActive: user.isActive,
      },
    });

    if (response.nextStep === 'shift_setup') {
      navigate('/shift/setup', { replace: true });
      return;
    }
    const permissions = permissionsForRole(user.role);
    const can = (permission: Permission) => permissions.includes(permission);
    navigate(getDefaultHomePath(can), { replace: true });
  };

  const handleResponse = (response: PanelLoginResponse) => {
    if (response.status === 'mfa_required') {
      setChallengeToken(response.challengeToken);
      setTotp('');
      setError(null);
      return;
    }
    completeLogin(response);
  };

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      const response = challengeToken
        ? await verifyPanelMfaAPI(challengeToken, trimValue(totp))
        : await loginPanelAPI(normalizeUsername(username), trimValue(password));
      handleResponse(response);
    } catch (caught) {
      const message = loginErrorMessage(caught);
      setError(message);
      if (isApiError(caught) && caught.message === 'AUTH_CHALLENGE_EXPIRED') {
        setChallengeToken(null);
        setTotp('');
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
    <Box component="form" onSubmit={handleSubmit} noValidate>
      <Typography variant="h4" fontWeight={700} gutterBottom>
        {challengeToken ? 'Verify your identity' : 'Welcome back'}
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
        {challengeToken
          ? 'Enter the six-digit code from your authenticator app.'
          : 'Use your Arena360 account. We’ll open the right workspace automatically.'}
      </Typography>

      {error ? (
        <Alert severity="error" sx={{ mb: 2 }}>
          {error}
        </Alert>
      ) : null}

      {!challengeToken ? (
        <>
          <UsernameField
            fullWidth
            label="Username"
            value={username}
            onChange={(event) => setUsername(event.target.value)}
            autoComplete="username"
            sx={{ mb: 2.5 }}
            required
          />
          <PasswordField
            fullWidth
            label="Password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            autoComplete="current-password"
            sx={{ mb: 1.5 }}
            required
          />
        </>
      ) : (
        <OtpField
          autoFocus
          fullWidth
          label="Authenticator code"
          value={totp}
          onChange={(event) => setTotp(event.target.value)}
          sx={{ mb: 1.5 }}
        />
      )}

      <FormButton
        type="submit"
        variant="contained"
        fullWidth
        size="large"
        disabled={submitting || (challengeToken ? totp.trim().length < 6 : !username || !password)}
        sx={{ py: 1.5, mb: 2, mt: 2 }}
      >
        {submitting ? 'Please wait…' : challengeToken ? 'Verify and continue' : 'Continue'}
      </FormButton>

      {challengeToken ? (
        <FormButton
          type="button"
          variant="text"
          fullWidth
          disabled={submitting}
          onClick={() => {
            setChallengeToken(null);
            setTotp('');
            setError(null);
            toastUtils.info('Enter your credentials to start again.');
          }}
        >
          Back to sign in
        </FormButton>
      ) : null}
    </Box>
  );
}
