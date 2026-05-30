import { Alert, Box, Button, FormControlLabel, Stack, Switch, TextField } from '@mui/material';
import { useState } from 'react';
import { AssetUploadField } from '../../components/AssetUploadField';
import type { GamePayload } from '../../services/game/add';
import type { GameResponse } from '../../services/game/list';

interface GameFormProps {
  initial?: GameResponse;
  submitLabel: string;
  loading?: boolean;
  error?: string;
  success?: string;
  onSubmit: (payload: GamePayload) => void;
  onCancel: () => void;
}

export function GameForm({
  initial,
  submitLabel,
  loading,
  error,
  success,
  onSubmit,
  onCancel,
}: GameFormProps) {
  const [name, setName] = useState(initial?.name ?? '');
  const [launchRef, setLaunchRef] = useState(initial?.launchRef ?? '');
  const [sortOrder, setSortOrder] = useState<number>(initial?.sortOrder ?? 0);
  const [isActive, setIsActive] = useState<boolean>(initial?.isActive ?? true);
  const [thumbnailUrl, setThumbnailUrl] = useState<string | null>(initial?.thumbnailUrl ?? null);
  const [logoUrl, setLogoUrl] = useState<string | null>(initial?.logoUrl ?? null);
  const [videoUrl, setVideoUrl] = useState<string | null>(initial?.videoUrl ?? null);
  const [nameError, setNameError] = useState(false);

  function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      setNameError(true);
      return;
    }
    onSubmit({
      name: name.trim(),
      launchRef: launchRef.trim() || null,
      sortOrder,
      isActive,
      thumbnailUrl,
      logoUrl,
      videoUrl,
    });
  }

  return (
    <Box component="form" onSubmit={handleSubmit}>
      {error ? (
        <Alert severity="error" sx={{ mb: 2 }}>
          {error}
        </Alert>
      ) : null}
      {success ? (
        <Alert severity="success" sx={{ mb: 2 }}>
          {success}
        </Alert>
      ) : null}

      <Stack spacing={3}>
        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2}>
          <TextField
            label="Game name"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setNameError(false);
            }}
            error={nameError}
            helperText={nameError ? 'Name is required' : 'Display name shown on the kiosk grid'}
            fullWidth
            required
          />
          <TextField
            label="Launch reference (optional)"
            value={launchRef}
            onChange={(e) => setLaunchRef(e.target.value)}
            helperText="Matches a kiosk allow-list entry by name/id (display-only otherwise)"
            fullWidth
          />
        </Stack>

        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={2} alignItems="center">
          <TextField
            label="Sort order"
            type="number"
            value={sortOrder}
            onChange={(e) => setSortOrder(Number.parseInt(e.target.value, 10) || 0)}
            sx={{ width: { xs: '100%', sm: 200 } }}
          />
          <FormControlLabel
            control={<Switch checked={isActive} onChange={(e) => setIsActive(e.target.checked)} />}
            label="Active (shown on kiosk)"
          />
        </Stack>

        <Stack direction={{ xs: 'column', sm: 'row' }} spacing={3}>
          <Box sx={{ flex: 1 }}>
            <AssetUploadField
              label="Thumbnail"
              kind="image"
              value={thumbnailUrl}
              onChange={setThumbnailUrl}
              helperText="Card cover image"
            />
          </Box>
          <Box sx={{ flex: 1 }}>
            <AssetUploadField
              label="Logo"
              kind="image"
              value={logoUrl}
              onChange={setLogoUrl}
              helperText="Transparent logo overlaid on the card"
            />
          </Box>
        </Stack>

        <AssetUploadField
          label="Background video"
          kind="video"
          value={videoUrl}
          onChange={setVideoUrl}
          helperText="Looped background; cached on-device for the login screen"
        />
      </Stack>

      <Stack direction="row" spacing={2} justifyContent="flex-end" sx={{ mt: 4 }}>
        <Button variant="outlined" onClick={onCancel} disabled={loading}>
          Cancel
        </Button>
        <Button type="submit" variant="contained" disabled={loading}>
          {loading ? 'Saving…' : submitLabel}
        </Button>
      </Stack>
    </Box>
  );
}
