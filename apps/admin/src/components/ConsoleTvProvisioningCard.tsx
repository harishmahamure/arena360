import TvIcon from '@mui/icons-material/Tv';
import {
  Alert,
  Box,
  Button,
  CircularProgress,
  Paper,
  Typography,
} from '@mui/material';
import { useState } from 'react';
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
  const [loading, setLoading] = useState(false);
  const [sent, setSent] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const isRegistered = registrationStatus === 'registered';

  const handleSendLogin = async () => {
    setLoading(true);
    setError(null);
    try {
      await createSsoToken({ purpose: 'tv_provision', deviceId });
      setSent(true);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to send TV login');
    } finally {
      setLoading(false);
    }
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
          <Button
            variant="contained"
            onClick={handleSendLogin}
            disabled={loading}
            startIcon={loading ? <CircularProgress size={18} color="inherit" /> : undefined}
          >
            Send TV login
          </Button>
          {sent && (
            <Alert severity="success" sx={{ mt: 2 }}>
              Token sent to station via WebSocket. The TV should advance automatically.
            </Alert>
          )}
          {error && (
            <Alert severity="error" sx={{ mt: 2 }}>
              {error}
            </Alert>
          )}
        </Box>
      </Box>
    </Paper>
  );
}
