import WarningAmberIcon from '@mui/icons-material/WarningAmber';
import { Box, Chip, Paper, Stack, Typography } from '@mui/material';

interface Fingerprint {
  mac?: string;
  serial?: string;
  biosUuid?: string;
  platform?: string;
  collectedAt?: string;
}

export interface KioskFingerprintCardProps {
  /** Raw `registeredKiosk` JSON string from the device record. */
  registeredKiosk?: string | null;
  registrationStatus?: string;
}

function parseFingerprint(raw: string | null | undefined): Fingerprint | null {
  if (!raw) return null;
  try {
    const value = JSON.parse(raw);
    if (value && typeof value === 'object') return value as Fingerprint;
    return null;
  } catch {
    return null;
  }
}

const FIELD_LABELS: Array<[keyof Fingerprint, string]> = [
  ['mac', 'MAC address'],
  ['serial', 'Serial number'],
  ['biosUuid', 'BIOS UUID'],
  ['platform', 'Platform'],
  ['collectedAt', 'Collected at'],
];

export function KioskFingerprintCard({
  registeredKiosk,
  registrationStatus,
}: KioskFingerprintCardProps) {
  if (registrationStatus !== 'registered') return null;

  const fingerprint = parseFingerprint(registeredKiosk);

  return (
    <Paper variant="outlined" sx={{ p: 3, mb: 3 }}>
      <Stack direction="row" alignItems="center" spacing={1} sx={{ mb: 2 }}>
        <Typography variant="h6" fontWeight={600}>
          Hardware fingerprint
        </Typography>
        {!fingerprint ? (
          <Chip
            size="small"
            color="warning"
            icon={<WarningAmberIcon />}
            label="No fingerprint on record"
          />
        ) : null}
      </Stack>

      {fingerprint ? (
        <Box
          sx={{
            display: 'grid',
            gridTemplateColumns: 'max-content 1fr',
            columnGap: 3,
            rowGap: 1,
          }}
        >
          {FIELD_LABELS.map(([key, label]) => (
            <Box key={key} sx={{ display: 'contents' }}>
              <Typography variant="body2" color="text.secondary">
                {label}
              </Typography>
              <Typography variant="body2" sx={{ fontFamily: 'monospace', wordBreak: 'break-all' }}>
                {fingerprint[key] ?? '—'}
              </Typography>
            </Box>
          ))}
        </Box>
      ) : (
        <Typography variant="body2" color="text.secondary">
          This kiosk is registered but has no stored hardware fingerprint. A drift check will
          re-capture it on the next player login.
        </Typography>
      )}
    </Paper>
  );
}
