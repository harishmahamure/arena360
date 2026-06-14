import { permissionsForRole } from '@gaming-cafe/contracts';
import { FormButton, FormTextField, PasswordField } from '@gaming-cafe/ui';
import { local, toastUtils } from '@gaming-cafe/utils';
import { Box, ToggleButton, ToggleButtonGroup, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { useDispatch } from '../../hooks/store';
import type { Permission } from '../../hooks/usePermissions';
import { loginAPI, loginStaffAPI } from '../../services/auth/auth';
import type { VerifyOtpResponseUser } from '../../services/auth/types';
import { getDefaultHomePath } from '../../utils/homePath';

type LoginMode = 'admin' | 'staff';

function isPanelRole(role: string): role is 'admin' | 'staff' {
  return role === 'admin' || role === 'staff';
}

export default function LoginPage() {
  const navigate = useNavigate();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [totp, setTotp] = useState('');
  const [loginMode, setLoginMode] = useState<LoginMode>('admin');
  const [panelLoginStep, setPanelLoginStep] = useState<1 | 2>(1);
  const [submitting, setSubmitting] = useState(false);
  const dispatch = useDispatch();

  const completeLogin = (accessToken: string, user: VerifyOtpResponseUser) => {
    if (!isPanelRole(user.role)) {
      toastUtils.error('Forbidden Access: User is not authorized for this panel');
      return;
    }

    local.set('accessToken', accessToken);
    dispatch({
      type: 'SetAuthDetail',
      payload: {
        ...user,
      },
    });
    const permissions = permissionsForRole(user.role);
    const can = (permission: Permission) => permissions.includes(permission);
    navigate(getDefaultHomePath(can));
  };

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);

    try {
      const totpCode = panelLoginStep === 2 ? totp : undefined;
      const response =
        loginMode === 'admin'
          ? await loginAPI(username, password, totpCode)
          : await loginStaffAPI(username, password, totpCode);
      completeLogin(response.accessToken, response.user);
    } catch (error: unknown) {
      const message =
        error instanceof Error ? error.message : 'Login failed. Please check your credentials.';

      if (panelLoginStep === 1 && message === 'TOTP code is required') {
        setPanelLoginStep(2);
        toastUtils.info('Please enter your authenticator code');
      } else {
        toastUtils.error(message);
      }
    } finally {
      setSubmitting(false);
    }
  };

  return (
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
            setPanelLoginStep(1);
            setTotp('');
          }
        }}
        sx={{ mb: 3 }}
      >
        <ToggleButton value="admin">Admin</ToggleButton>
        <ToggleButton value="staff">Staff</ToggleButton>
      </ToggleButtonGroup>

      {panelLoginStep === 1 && (
        <>
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
        </>
      )}

      {panelLoginStep === 2 && (
        <FormTextField
          fullWidth
          variant="outlined"
          label="TOTP Code"
          type="text"
          value={totp}
          onChange={(e) => setTotp(e.target.value.replace(/\s+/g, '').slice(0, 6))}
          sx={{ mb: 1.5 }}
          inputProps={{
            autoComplete: 'one-time-code',
          }}
        />
      )}

      <FormButton
        type="submit"
        variant="contained"
        fullWidth
        size="large"
        disabled={submitting}
        sx={{ py: 1.5, mb: 3, mt: 2 }}
      >
        {panelLoginStep === 2 ? 'Verify TOTP' : 'Sign in'}
      </FormButton>

      {panelLoginStep === 2 && (
        <FormButton
          variant="text"
          fullWidth
          onClick={() => setPanelLoginStep(1)}
          disabled={submitting}
          sx={{ mb: 3 }}
        >
          Back to login
        </FormButton>
      )}
    </Box>
  );
}
