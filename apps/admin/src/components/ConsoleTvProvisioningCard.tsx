import { FormButton } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import LoginIcon from '@mui/icons-material/Login';
import SyncIcon from '@mui/icons-material/Sync';
import TvIcon from '@mui/icons-material/Tv';
import {
  Alert,
  Box,
  List,
  ListItem,
  ListItemIcon,
  ListItemText,
  Paper,
  Typography,
} from '@mui/material';
import QRCode from 'react-qr-code';
import { createSsoToken } from '../services/auth/createSsoToken';

export interface ConsoleTvProvisioningCardProps {
  deviceId: string;
  deviceName?: string;
  registrationStatus?: string;
}

const PROVISIONING_STEPS = [
  {
    icon: <TvIcon fontSize="small" />,
    primary: 'Enter the device ID on the Android TV',
    secondary:
      'On the station, type or scan the device ID shown below, then tap Connect & wait for SSO.',
  },
  {
    icon: <SyncIcon fontSize="small" />,
    primary: 'Wait for the TV to connect',
    secondary: 'The TV opens a WebSocket and shows “Waiting for admin to send TV login…”.',
  },
  {
    icon: <LoginIcon fontSize="small" />,
    primary: 'Send TV login from admin',
    secondary:
      'Click the button below. The TV receives the token over WebSocket and advances automatically.',
  },
  {
    icon: <CheckCircleOutlineIcon fontSize="small" />,
    primary: 'Complete station details on the TV',
    secondary:
      'On the TV, confirm the station name, device type, and location, then tap Provision station.',
  },
] as const;

export function ConsoleTvProvisioningCard({
  deviceId,
  deviceName,
  registrationStatus,
}: ConsoleTvProvisioningCardProps) {
  const { loading, succeeded, failed, errorMessage, run } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });

  const isRegistered = registrationStatus === 'registered';

  const handleSendLogin = () => {
    void run(async () => {
      await createSsoToken({ purpose: 'tv_provision', deviceId });
    });
  };

  if (isRegistered) {
    return (
      <Alert severity="success" sx={{ mb: 3 }}>
        This PlayStation station is registered and ready for admin-started sessions.
      </Alert>
    );
  }

  return (
    <Paper variant="outlined" sx={{ p: 3, mb: 3 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1 }}>
        <TvIcon color="primary" />
        <Typography variant="h6" fontWeight={600}>
          Android TV provisioning
        </Typography>
      </Box>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        {deviceName
          ? `Provision "${deviceName}" by pairing the TV with this device record.`
          : 'Pair the Android TV with this device record using the steps below.'}
      </Typography>

      <Alert severity="info" sx={{ mb: 2 }}>
        Ensure device type, sub type, and location in the form below match what you enter on the TV.
        Staff must send TV login while the TV is waiting on step 1.
      </Alert>

      <List dense disablePadding sx={{ mb: 2 }}>
        {PROVISIONING_STEPS.map((step) => (
          <ListItem key={step.primary} disableGutters sx={{ alignItems: 'flex-start', py: 0.75 }}>
            <ListItemIcon sx={{ minWidth: 36, mt: 0.25 }}>{step.icon}</ListItemIcon>
            <ListItemText
              primary={
                <Typography variant="body2" fontWeight={600}>
                  {step.primary}
                </Typography>
              }
              secondary={step.secondary}
            />
          </ListItem>
        ))}
      </List>

      <Box sx={{ display: 'flex', gap: 3, flexWrap: 'wrap', alignItems: 'flex-start' }}>
        <Box>
          <Typography variant="caption" color="text.secondary">
            Device ID (pairing)
          </Typography>
          <Typography variant="body2" fontFamily="monospace" sx={{ mb: 1 }}>
            {deviceId}
          </Typography>
          <QRCode value={deviceId} size={120} />
        </Box>
        <Box sx={{ flex: 1, minWidth: 220 }}>
          <FormButton
            variant="contained"
            onClick={handleSendLogin}
            loading={loading}
            success={succeeded}
            successLabel="Login sent"
            error={failed}
            errorLabel={errorMessage ?? 'Failed to send TV login'}
          >
            Send TV login
          </FormButton>
          {succeeded && (
            <Alert severity="success" sx={{ mt: 2 }}>
              Token sent to station via WebSocket. The TV should advance automatically.
            </Alert>
          )}
        </Box>
      </Box>

      {registrationStatus && registrationStatus !== 'registered' && (
        <Box sx={{ mt: 2 }}>
          <Typography variant="caption" color="text.secondary">
            Current status: {registrationStatus}
          </Typography>
        </Box>
      )}
    </Paper>
  );
}
