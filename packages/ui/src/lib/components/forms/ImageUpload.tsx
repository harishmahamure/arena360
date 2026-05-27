'use client';

import { CloudUpload, Delete, Image as ImageIcon } from '@mui/icons-material';
import { Box, Button, Card, CardActions, CardMedia, IconButton, Typography } from '@mui/material';
import { forwardRef, useRef, useState } from 'react';

export interface ImageUploadProps {
  /**
   * Callback when image is selected
   */
  onChange?: (file: File | null) => void;
  /**
   * Current image URL or File
   */
  value?: string | File | null;
  /**
   * Maximum file size in bytes
   */
  maxSize?: number;
  /**
   * Width of the preview image
   */
  previewWidth?: number | string;
  /**
   * Height of the preview image
   */
  previewHeight?: number | string;
  /**
   * If true, shows an error state
   */
  error?: boolean;
  /**
   * Error message to display
   */
  errorMessage?: string;
  /**
   * Helper text to display
   */
  helperText?: string;
  /**
   * If true, component is disabled
   */
  disabled?: boolean;
  /**
   * Accept attribute for file input
   * @default "image/*"
   */
  accept?: string;
  /**
   * Button label
   */
  buttonLabel?: string;
}

/**
 * ImageUpload - An image upload component with preview
 */
const ImageUpload = forwardRef<HTMLDivElement, ImageUploadProps>(
  (
    {
      onChange,
      value,
      maxSize = 5 * 1024 * 1024, // 5MB default
      previewWidth = 300,
      previewHeight = 200,
      error = false,
      errorMessage,
      helperText,
      disabled = false,
      accept = 'image/*',
      buttonLabel = 'Upload Image',
    },
    ref,
  ) => {
    const inputRef = useRef<HTMLInputElement>(null);
    const [previewUrl, setPreviewUrl] = useState<string | null>(null);

    const handleClick = () => {
      inputRef.current?.click();
    };

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const file = e.target.files?.[0];

      if (file) {
        if (maxSize && file.size > maxSize) {
          return;
        }

        // Create preview URL
        const url = URL.createObjectURL(file);
        setPreviewUrl(url);
        onChange?.(file);
      }

      // Reset input
      if (inputRef.current) {
        inputRef.current.value = '';
      }
    };

    const handleRemove = () => {
      setPreviewUrl(null);
      onChange?.(null);
      if (inputRef.current) {
        inputRef.current.value = '';
      }
    };

    const getImageUrl = (): string | null => {
      if (previewUrl) return previewUrl;
      if (typeof value === 'string') return value;
      if (value instanceof File) return URL.createObjectURL(value);
      return null;
    };

    const imageUrl = getImageUrl();

    return (
      <Box ref={ref}>
        <input
          ref={inputRef}
          type="file"
          accept={accept}
          onChange={handleChange}
          style={{ display: 'none' }}
          disabled={disabled}
        />

        {imageUrl ? (
          <Card sx={{ maxWidth: previewWidth, mb: 1 }}>
            <CardMedia
              component="img"
              height={previewHeight}
              image={imageUrl}
              alt="Preview"
              sx={{ objectFit: 'contain', bgcolor: 'grey.100' }}
            />
            <CardActions>
              <Button
                size="small"
                startIcon={<CloudUpload />}
                onClick={handleClick}
                disabled={disabled}
              >
                Change
              </Button>
              <IconButton size="small" onClick={handleRemove} disabled={disabled} color="error">
                <Delete />
              </IconButton>
            </CardActions>
          </Card>
        ) : (
          <Box
            sx={{
              width: previewWidth,
              height: previewHeight,
              border: 2,
              borderColor: error ? 'error.main' : 'grey.300',
              borderStyle: 'dashed',
              borderRadius: 1,
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              bgcolor: 'grey.50',
              cursor: disabled ? 'not-allowed' : 'pointer',
              '&:hover': {
                bgcolor: disabled ? 'grey.50' : 'grey.100',
              },
            }}
            onClick={disabled ? undefined : handleClick}
          >
            <ImageIcon sx={{ fontSize: 48, color: 'grey.400', mb: 1 }} />
            <Button
              variant="outlined"
              startIcon={<CloudUpload />}
              disabled={disabled}
              color={error ? 'error' : 'primary'}
            >
              {buttonLabel}
            </Button>
          </Box>
        )}

        {helperText && !error && (
          <Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 0.5 }}>
            {helperText}
          </Typography>
        )}

        {error && errorMessage && (
          <Typography variant="caption" color="error" display="block" sx={{ mt: 0.5 }}>
            {errorMessage}
          </Typography>
        )}
      </Box>
    );
  },
);

ImageUpload.displayName = 'ImageUpload';

export default ImageUpload;
