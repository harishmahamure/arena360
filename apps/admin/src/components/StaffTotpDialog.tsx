import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  TextField,
} from '@mui/material';
import { useEffect, useState } from 'react';

interface StaffTotpDialogProps {
  open: boolean;
  title: string;
  description: string;
  confirmLabel?: string;
  loading?: boolean;
  onClose: () => void;
  onConfirm: (totp: string) => void | Promise<void>;
}

export function StaffTotpDialog({
  open,
  title,
  description,
  confirmLabel = 'Confirm',
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
    const code = totp.trim();
    if (!code) return;
    void onConfirm(code);
  };

  return (
    <Dialog open={open} onClose={loading ? undefined : onClose} fullWidth maxWidth="xs">
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        <DialogContentText sx={{ mb: 2 }}>{description}</DialogContentText>
        <TextField
          autoFocus
          fullWidth
          label="Authenticator code"
          value={totp}
          onChange={(e) => setTotp(e.target.value)}
          inputProps={{ autoComplete: 'one-time-code', inputMode: 'numeric' }}
          onKeyDown={(e) => {
            if (e.key === 'Enter') {
              e.preventDefault();
              handleConfirm();
            }
          }}
        />
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose} disabled={loading}>
          Cancel
        </Button>
        <Button variant="contained" onClick={handleConfirm} disabled={loading || !totp.trim()}>
          {confirmLabel}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
