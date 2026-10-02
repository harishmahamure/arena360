import { OtpField } from '@gaming-cafe/ui';
import { trimValue } from '@gaming-cafe/utils';
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  Typography,
} from '@mui/material';
import { useEffect, useState } from 'react';

interface StaffTotpDialogProps {
  open: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  confirmColor?: 'primary' | 'error';
  loading?: boolean;
  onClose: () => void;
  onConfirm: (totp: string) => void | Promise<void>;
}

export function StaffTotpDialog({
  open,
  title,
  description,
  confirmLabel = 'Confirm',
  confirmColor = 'primary',
  loading = false,
  onClose,
  onConfirm,
}: StaffTotpDialogProps) {
  const [totp, setTotp] = useState('');

  useEffect(() => {
    if (!open) {
      setTotp('');
    }
  }, [open]);

  const handleConfirm = () => {
    const code = trimValue(totp);
    if (!code) return;
    void onConfirm(code);
  };

  return (
    <Dialog open={open} onClose={loading ? undefined : onClose} fullWidth maxWidth="xs">
      <DialogTitle sx={{ pb: 1 }}>
        <Typography variant="subtitle1" component="div" fontWeight={600}>
          {title}
        </Typography>
      </DialogTitle>
      <DialogContent>
        <DialogContentText sx={{ mb: 2 }}>{description}</DialogContentText>
        <OtpField
          autoFocus
          fullWidth
          size="small"
          label="Authenticator code"
          placeholder="6-digit code"
          helperText="Enter the code from your authenticator app"
          value={totp}
          onChange={(e) => setTotp(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              handleConfirm();
            }
          }}
        />
      </DialogContent>
      <DialogActions sx={{ px: 3, pb: 2 }}>
        <Button variant="text" onClick={onClose} disabled={loading}>
          Cancel
        </Button>
        <Button
          variant="contained"
          color={confirmColor}
          onClick={handleConfirm}
          disabled={loading || !totp.trim()}
        >
          {confirmLabel}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
