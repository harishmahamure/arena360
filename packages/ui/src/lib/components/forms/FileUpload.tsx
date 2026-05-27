'use client';

import { Close, CloudUpload, InsertDriveFile } from '@mui/icons-material';
import { Box, Button, Chip, Typography } from '@mui/material';
import { forwardRef, useRef } from 'react';

export interface FileUploadProps {
  /**
   * Accept attribute for file input
   * @example "image/*" or ".pdf,.doc,.docx"
   */
  accept?: string;
  /**
   * If true, allows multiple file selection
   */
  multiple?: boolean;
  /**
   * Maximum file size in bytes
   */
  maxSize?: number;
  /**
   * Callback when files are selected
   */
  onChange?: (files: File[]) => void;
  /**
   * Callback when files are removed
   */
  onRemove?: (index: number) => void;
  /**
   * Currently selected files
   */
  files?: File[];
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
   * Button label
   */
  buttonLabel?: string;
  /**
   * If true, component is disabled
   */
  disabled?: boolean;
  /**
   * Variant of the upload button
   */
  variant?: 'contained' | 'outlined' | 'text';
}

/**
 * FileUpload - A file upload component with file list display
 */
const FileUpload = forwardRef<HTMLDivElement, FileUploadProps>(
  (
    {
      accept,
      multiple = false,
      maxSize,
      onChange,
      onRemove,
      files = [],
      error = false,
      errorMessage,
      helperText,
      buttonLabel = 'Upload File',
      disabled = false,
      variant = 'outlined',
    },
    ref,
  ) => {
    const inputRef = useRef<HTMLInputElement>(null);

    const handleClick = () => {
      inputRef.current?.click();
    };

    const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      const selectedFiles = Array.from(e.target.files || []);

      if (maxSize) {
        const validFiles = selectedFiles.filter((file) => {
          if (file.size > maxSize) {
            return false;
          }
          return true;
        });
        onChange?.(validFiles);
      } else {
        onChange?.(selectedFiles);
      }

      // Reset input value to allow selecting the same file again
      if (inputRef.current) {
        inputRef.current.value = '';
      }
    };

    const handleRemove = (index: number) => {
      onRemove?.(index);
    };

    const formatFileSize = (bytes: number): string => {
      if (bytes === 0) return '0 Bytes';
      const k = 1024;
      const sizes = ['Bytes', 'KB', 'MB', 'GB'];
      const i = Math.floor(Math.log(bytes) / Math.log(k));
      return `${Math.round((bytes / k ** i) * 100) / 100} ${sizes[i]}`;
    };

    return (
      <Box ref={ref}>
        <input
          ref={inputRef}
          type="file"
          accept={accept}
          multiple={multiple}
          onChange={handleChange}
          style={{ display: 'none' }}
          disabled={disabled}
        />

        <Button
          variant={variant}
          startIcon={<CloudUpload />}
          onClick={handleClick}
          disabled={disabled}
          color={error ? 'error' : 'primary'}
        >
          {buttonLabel}
        </Button>

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

        {files.length > 0 && (
          <Box sx={{ mt: 2 }}>
            {files.map((file, index) => (
              <Chip
                key={`${file.name}-${file.size}-${file.lastModified}`}
                icon={<InsertDriveFile />}
                label={`${file.name} (${formatFileSize(file.size)})`}
                onDelete={() => handleRemove(index)}
                deleteIcon={<Close />}
                sx={{ mb: 1, mr: 1, maxWidth: '100%' }}
              />
            ))}
          </Box>
        )}
      </Box>
    );
  },
);

FileUpload.displayName = 'FileUpload';

export default FileUpload;
