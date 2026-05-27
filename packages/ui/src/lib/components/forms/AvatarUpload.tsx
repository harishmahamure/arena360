'use client';

import { Delete, PhotoCamera } from '@mui/icons-material';
import { Avatar, Badge, Box, IconButton, Typography } from '@mui/material';
import { forwardRef, useRef, useState } from 'react';

export interface AvatarUploadProps {
  /**
   * Callback when image is selected
   */
  onChange?: (file: File | null) => void;
  /**
   * Current avatar URL or File
   */
  value?: string | File | null;
  /**
   * Maximum file size in bytes
   */
  maxSize?: number;
  /**
   * Size of the avatar
   */
  size?: number;
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
   * Alt text for avatar
   */
  alt?: string;
  /**
   * If true, shows delete button
   */
  showDelete?: boolean;
}

/**
 * AvatarUpload - An avatar/profile picture upload component
 */
const AvatarUpload = forwardRef<HTMLDivElement, AvatarUploadProps>(
  (
    {
      onChange,
      value,
      maxSize = 5 * 1024 * 1024, // 5MB default
      size = 120,
      error = false,
      errorMessage,
      helperText,
      disabled = false,
      accept = 'image/*',
      alt = 'Avatar',
      showDelete = true,
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

    const handleRemove = (e: React.MouseEvent) => {
      e.stopPropagation();
      setPreviewUrl(null);
      onChange?.(null);
      if (inputRef.current) {
        inputRef.current.value = '';
      }
    };

    const getAvatarUrl = (): string | undefined => {
      if (previewUrl) return previewUrl;
      if (typeof value === 'string') return value;
      if (value instanceof File) return URL.createObjectURL(value);
      return undefined;
    };

    const avatarUrl = getAvatarUrl();

    return (
      <Box ref={ref} sx={{ display: 'inline-flex', flexDirection: 'column', alignItems: 'center' }}>
        <input
          ref={inputRef}
          type="file"
          accept={accept}
          onChange={handleChange}
          style={{ display: 'none' }}
          disabled={disabled}
        />

        <Badge
          overlap="circular"
          anchorOrigin={{ vertical: 'bottom', horizontal: 'right' }}
          badgeContent={
            <Box>
              <IconButton
                size="small"
                onClick={handleClick}
                disabled={disabled}
                sx={{
                  bgcolor: 'primary.main',
                  color: 'white',
                  '&:hover': { bgcolor: 'primary.dark' },
                  width: 36,
                  height: 36,
                }}
              >
                <PhotoCamera fontSize="small" />
              </IconButton>
              {showDelete && avatarUrl && (
                <IconButton
                  size="small"
                  onClick={handleRemove}
                  disabled={disabled}
                  sx={{
                    bgcolor: 'error.main',
                    color: 'white',
                    '&:hover': { bgcolor: 'error.dark' },
                    width: 36,
                    height: 36,
                    ml: 0.5,
                  }}
                >
                  <Delete fontSize="small" />
                </IconButton>
              )}
            </Box>
          }
        >
          <Avatar
            src={avatarUrl}
            alt={alt}
            sx={{
              width: size,
              height: size,
              border: 3,
              borderColor: error ? 'error.main' : 'grey.300',
              cursor: disabled ? 'not-allowed' : 'pointer',
              '&:hover': {
                opacity: disabled ? 1 : 0.8,
              },
            }}
            onClick={disabled ? undefined : handleClick}
          />
        </Badge>

        {helperText && !error && (
          <Typography variant="caption" color="text.secondary" sx={{ mt: 1, textAlign: 'center' }}>
            {helperText}
          </Typography>
        )}

        {error && errorMessage && (
          <Typography variant="caption" color="error" sx={{ mt: 1, textAlign: 'center' }}>
            {errorMessage}
          </Typography>
        )}
      </Box>
    );
  },
);

AvatarUpload.displayName = 'AvatarUpload';

export default AvatarUpload;
