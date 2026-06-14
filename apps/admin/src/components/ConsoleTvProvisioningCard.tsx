import { FormButton } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import TvIcon from '@mui/icons-material/Tv';
import { Alert, Box, Paper, Typography } from '@mui/material';
import QRCode from 'react-qr-code';
import { createSsoToken } from '../services/auth/createSsoToken';

export interface ConsoleTvProvisioningCardProps {
  deviceId: string;
  deviceName?: string;
  registrationStatus?: string;
}

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
          ? `On "${deviceName}", enter this device ID on the TV, then send login.`
          : 'Enter this device ID on the TV, then send login.'}
      </Typography>

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
    </Paper>
  );
}
