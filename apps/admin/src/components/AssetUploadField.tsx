import { ImageUpload } from '@gaming-cafe/ui';
import { CloudUpload, Delete } from '@mui/icons-material';
import { Box, Button, IconButton, Stack, Typography } from '@mui/material';
import { useRef, useState } from 'react';
import { uploadAsset } from '../services/upload/presign';

interface AssetUploadFieldProps {
  label: string;
  value: string | null;
  onChange: (url: string | null) => void;
  kind?: 'image' | 'video';
  helperText?: string;
}

/**
 * Uploads an asset to object storage via a presigned PUT (DRAFT-0022) and emits
 * the resulting public URL. Images preview inline; videos show a small player.
 */
export function AssetUploadField({
  label,
  value,
  onChange,
  kind = 'image',
  helperText,
}: AssetUploadFieldProps) {
  const [uploading, setUploading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const videoInputRef = useRef<HTMLInputElement>(null);

  async function handleFile(file: File | null) {
    if (!file) {
      onChange(null);
      return;
    }
    setUploading(true);
    setError(undefined);
    try {
      const url = await uploadAsset(file);
      onChange(url);
    } catch (e) {
      setError(e instanceof Error ? e.message : 'Upload failed');
    } finally {
      setUploading(false);
    }
  }

  if (kind === 'image') {
    return (
      <Box>
        <Typography variant="subtitle2" gutterBottom>
          {label}
        </Typography>
        <ImageUpload
          value={value}
          onChange={handleFile}
          accept="image/*"
          disabled={uploading}
          helperText={uploading ? 'Uploading…' : helperText}
          error={Boolean(error)}
          errorMessage={error}
          buttonLabel={`Upload ${label}`}
          previewHeight={140}
        />
      </Box>
    );
  }

  return (
    <Box>
      <Typography variant="subtitle2" gutterBottom>
        {label}
      </Typography>
      <input
        ref={videoInputRef}
        type="file"
        accept="video/*"
        style={{ display: 'none' }}
        onChange={(e) => void handleFile(e.target.files?.[0] ?? null)}
      />
      {value ? (
        <Stack spacing={1}>
          <video
            src={value}
            controls
            muted
            style={{ width: '100%', maxWidth: 300, borderRadius: 8, background: '#000' }}
          />
          <Stack direction="row" spacing={1}>
            <Button
              size="small"
              startIcon={<CloudUpload />}
              onClick={() => videoInputRef.current?.click()}
              disabled={uploading}
            >
              Change
            </Button>
            <IconButton size="small" color="error" onClick={() => onChange(null)}>
              <Delete />
            </IconButton>
          </Stack>
        </Stack>
      ) : (
        <Button
          variant="outlined"
          startIcon={<CloudUpload />}
          onClick={() => videoInputRef.current?.click()}
          disabled={uploading}
          color={error ? 'error' : 'primary'}
        >
          {uploading ? 'Uploading…' : `Upload ${label}`}
        </Button>
      )}
      {(helperText || error) && (
        <Typography
          variant="caption"
          color={error ? 'error' : 'text.secondary'}
          display="block"
          sx={{ mt: 0.5 }}
        >
          {error ?? helperText}
        </Typography>
      )}
    </Box>
  );
}
