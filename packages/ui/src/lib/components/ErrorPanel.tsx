'use client';

import { Alert, Box, Button } from '@mui/material';

export interface ErrorPanelProps {
  message: string;
  onRetry?: () => void;
  retryLabel?: string;
}

export function ErrorPanel({ message, onRetry, retryLabel = 'Retry' }: ErrorPanelProps) {
  return (
    <Box>
      <Alert severity="error">{message}</Alert>
      {onRetry && (
        <Button onClick={onRetry} sx={{ mt: 2 }} variant="outlined">
          {retryLabel}
        </Button>
      )}
    </Box>
  );
}
