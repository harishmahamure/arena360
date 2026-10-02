import { Box, Typography } from '@mui/material';
import QRCode from 'react-qr-code';

interface TotpQrCodeProps {
  value: string;
  accountLabel?: string;
  secret?: string;
}

export default function TotpQrCode({ value, accountLabel, secret }: TotpQrCodeProps) {
  return (
    <Box sx={{ display: 'grid', gap: 2 }}>
      <Typography variant="body2" color="text.secondary">
        Scan this QR code with an authenticator app
        {accountLabel ? ` for ${accountLabel}` : ''} (Google Authenticator, Authy, etc.).
      </Typography>
      <Box
        sx={{
          p: 2,
          bgcolor: 'background.paper',
          border: 1,
          borderColor: 'divider',
          borderRadius: 1,
          width: 'fit-content',
        }}
      >
        <QRCode value={value} size={200} level="M" />
      </Box>
      {secret && (
        <Box>
          <Typography variant="caption" color="text.secondary" display="block" gutterBottom>
            Manual entry key
          </Typography>
          <Typography
            variant="body2"
            sx={{
              fontFamily: 'monospace',
              wordBreak: 'break-all',
              userSelect: 'all',
            }}
          >
            {secret}
          </Typography>
        </Box>
      )}
    </Box>
  );
}
