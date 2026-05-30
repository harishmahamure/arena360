import ContentCopyIcon from '@mui/icons-material/ContentCopy';
import RefreshIcon from '@mui/icons-material/Refresh';
import { Alert, Box, Button, Paper, Stack, Typography } from '@mui/material';
import { useEffect, useState } from 'react';
import { toast } from 'react-toastify';
import { issueRegistrationCode } from '../services/devices/issueRegistrationCode';

function isCodeExpired(expiresAt: string | null | undefined): boolean {
  if (!expiresAt) return true;
  return new Date(expiresAt).getTime() <= Date.now();
}

function formatExpiry(expiresAt: string): string {
  return new Date(expiresAt).toLocaleString();
}

export interface KioskRegistrationCardProps {
  deviceId: string;
  registrationStatus?: string;
  registrationCode?: string | null;
  expiresAt?: string | null;
  canWrite: boolean;
  onCodeUpdated?: () => void;
}

export function KioskRegistrationCard({
  deviceId,
  registrationStatus,
  registrationCode,
  expiresAt,
  canWrite,
  onCodeUpdated,
}: KioskRegistrationCardProps) {
  const [code, setCode] = useState(registrationCode ?? null);
  const [expiry, setExpiry] = useState(expiresAt ?? null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setCode(registrationCode ?? null);
    setExpiry(expiresAt ?? null);
  }, [registrationCode, expiresAt]);

  const isRegistered = registrationStatus === 'registered';
  const hasValidCode = code && expiry && !isCodeExpired(expiry);

  const handleCopy = async () => {
    if (!code) return;
    try {
      await navigator.clipboard.writeText(code);
      toast.success('Registration code copied');
    } catch {
      toast.error('Failed to copy code');
    }
  };

  const handleGenerate = async () => {
    setLoading(true);
    try {
      const result = await issueRegistrationCode(deviceId);
      setCode(result.registrationCode);
      setExpiry(result.expiresAt);
      toast.success('Registration code generated');
      onCodeUpdated?.();
    } catch (err) {
      toast.error(err instanceof Error ? err.message : 'Failed to generate code');
    } finally {
      setLoading(false);
    }
  };

  if (isRegistered) {
    return (
      <Alert severity="success" sx={{ mb: 3 }}>
        This kiosk is registered. Registration codes are no longer shown.
      </Alert>
    );
  }

  return (
    <Paper variant="outlined" sx={{ p: 3, mb: 3 }}>
      <Typography variant="h6" fontWeight={600} gutterBottom>
        Kiosk registration
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        Enter this one-time code on the kiosk registration screen. Codes expire after 24 hours.
      </Typography>

      {hasValidCode ? (
        <Stack spacing={2}>
          <Box
            sx={{
              p: 2,
              bgcolor: 'action.hover',
              borderRadius: 1,
              textAlign: 'center',
            }}
          >
            <Typography
              variant="h4"
              component="p"
              sx={{ fontFamily: 'monospace', letterSpacing: 4, fontWeight: 700 }}
            >
              {code}
            </Typography>
            <Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 1 }}>
              Expires {formatExpiry(expiry)}
            </Typography>
          </Box>
          <Stack direction="row" spacing={1} flexWrap="wrap">
            <Button
              variant="contained"
              startIcon={<ContentCopyIcon />}
              onClick={() => void handleCopy()}
            >
              Copy code
            </Button>
            {canWrite && (
              <Button
                variant="outlined"
                startIcon={<RefreshIcon />}
                disabled={loading}
                onClick={() => void handleGenerate()}
              >
                Regenerate code
              </Button>
            )}
          </Stack>
        </Stack>
      ) : (
        <Stack spacing={2}>
          <Alert severity="warning">
            {code && isCodeExpired(expiry)
              ? 'The registration code has expired. Generate a new code for the kiosk.'
              : 'No active registration code. Generate one before pairing the kiosk.'}
          </Alert>
          {canWrite && (
            <Button
              variant="contained"
              startIcon={<RefreshIcon />}
              disabled={loading}
              onClick={() => void handleGenerate()}
            >
              Generate code
            </Button>
          )}
        </Stack>
      )}
    </Paper>
  );
}
