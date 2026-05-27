'use client';

import { FormButton } from '@gaming-cafe/ui';
import CloseIcon from '@mui/icons-material/Close';
import {
  Box,
  Dialog,
  DialogContent,
  DialogTitle,
  IconButton,
  TextField,
  Typography,
} from '@mui/material';
import { type ClipboardEvent, type KeyboardEvent, useRef, useState } from 'react';

interface OtpModalProps {
  open: boolean;
  onClose: () => void;
  onVerify: (otp: string) => Promise<void>;
  loading?: boolean;
}

const OTP_SLOT_IDS = ['otp-0', 'otp-1', 'otp-2', 'otp-3', 'otp-4', 'otp-5'] as const;

export default function OtpModal({ open, onClose, onVerify, loading = false }: OtpModalProps) {
  const [otp, setOtp] = useState<string[]>(['', '', '', '', '', '']);
  const inputRefs = useRef<(HTMLInputElement | null)[]>([]);

  const handleChange = (index: number, value: string) => {
    if (value && !/^\d$/.test(value)) return;

    const newOtp = [...otp];
    newOtp[index] = value;
    setOtp(newOtp);

    // Auto-focus next input
    if (value && index < 5) {
      inputRefs.current[index + 1]?.focus();
    }
  };

  const handleKeyDown = (index: number, e: KeyboardEvent<HTMLDivElement>) => {
    if (e.key === 'Backspace' && !otp[index] && index > 0) {
      inputRefs.current[index - 1]?.focus();
    }
  };

  const handlePaste = (e: ClipboardEvent<HTMLDivElement>) => {
    e.preventDefault();
    const pastedData = e.clipboardData.getData('text/plain');
    const pastedOtp = pastedData.slice(0, 6).split('');

    if (pastedOtp.every((char) => /^\d$/.test(char))) {
      const newOtp = [...otp];
      pastedOtp.forEach((char, index) => {
        if (index < 6) {
          newOtp[index] = char;
        }
      });
      setOtp(newOtp);
      // Focus last filled input
      const lastIndex = Math.min(pastedOtp.length - 1, 5);
      inputRefs.current[lastIndex]?.focus();
    }
  };

  const handleSubmit = async () => {
    const otpString = otp.join('');
    if (otpString.length === 6) {
      await onVerify(otpString);
    }
  };

  const handleModalClose = () => {
    if (!loading) {
      setOtp(['', '', '', '', '', '']);
      onClose();
    }
  };

  return (
    <Dialog
      open={open}
      onClose={handleModalClose}
      maxWidth="sm"
      fullWidth
      PaperProps={{
        sx: {
          borderRadius: 2,
          px: 2,
          py: 4,
          height: '360px',
          display: 'flex',
          flexDirection: 'column',
          justifyContent: 'center',
          alignItems: 'center',
        },
      }}
    >
      <IconButton onClick={handleModalClose} sx={{ position: 'absolute', right: 16, top: 16 }}>
        <CloseIcon />
      </IconButton>

      <DialogTitle sx={{ position: 'relative', pb: 1 }}>
        <Typography variant="h5" fontWeight={700}>
          Enter OTP
        </Typography>
        <IconButton
          aria-label="close"
          onClick={handleModalClose}
          disabled={loading}
          sx={{
            position: 'absolute',
            right: 16,
            top: 16,
            color: (theme) => theme.palette.grey[500],
          }}
        ></IconButton>
      </DialogTitle>
      <DialogContent>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 4 }}>
          Please enter the 6-digit OTP sent to your registered email
        </Typography>

        <Box
          sx={{
            display: 'flex',
            gap: 1.5,
            justifyContent: 'center',
            mb: 4,
          }}
          onPaste={handlePaste}
        >
          {OTP_SLOT_IDS.map((slotId, index) => (
            <TextField
              key={slotId}
              inputRef={(el) => (inputRefs.current[index] = el)}
              value={otp[index]}
              onChange={(e) => handleChange(index, e.target.value)}
              onKeyDown={(e) => handleKeyDown(index, e)}
              inputProps={{
                maxLength: 1,
                style: {
                  textAlign: 'center',
                  fontSize: '1.5rem',
                  fontWeight: 600,
                },
              }}
              sx={{
                width: 56,
                '& .MuiOutlinedInput-root': {
                  '& fieldset': {
                    borderWidth: 2,
                  },
                  '&:hover fieldset': {
                    borderWidth: 2,
                  },
                  '&.Mui-focused fieldset': {
                    borderWidth: 2,
                  },
                },
              }}
            />
          ))}
        </Box>

        <FormButton
          type="button"
          variant="contained"
          fullWidth
          size="large"
          onClick={handleSubmit}
          disabled={otp.join('').length !== 6 || loading}
          sx={{ py: 1.5 }}
        >
          {loading ? 'Verifying...' : 'Verify OTP'}
        </FormButton>
      </DialogContent>
    </Dialog>
  );
}
