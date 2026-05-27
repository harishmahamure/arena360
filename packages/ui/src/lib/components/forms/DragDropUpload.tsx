'use client';

import { Close, CloudUpload, InsertDriveFile } from '@mui/icons-material';
import {
  Box,
  IconButton,
  List,
  ListItem,
  ListItemIcon,
  ListItemText,
  Paper,
  Typography,
} from '@mui/material';
import { forwardRef, useRef, useState } from 'react';

export interface DragDropUploadProps {
  /**
   * Accept attribute for file input
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
   * Maximum number of files
   */
  maxFiles?: number;
  /**
   * Callback when files are added
   */
  onChange?: (files: File[]) => void;
  /**
   * Callback when a file is removed
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
   * If true, component is disabled
   */
  disabled?: boolean;
  /**
   * Height of the drop zone
   */
  height?: number | string;
}

/**
 * DragDropUpload - A drag and drop file upload component
 */
const DragDropUpload = forwardRef<HTMLDivElement, DragDropUploadProps>(
  (
    {
      accept,
      multiple = true,
      maxSize,
      maxFiles,
      onChange,
      onRemove,
      files = [],
      error = false,
      errorMessage,
      helperText,
      disabled = false,
      height = 200,
    },
    ref,
  ) => {
    const [isDragging, setIsDragging] = useState(false);
    const inputRef = useRef<HTMLInputElement>(null);

    const handleDragEnter = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (!disabled) {
        setIsDragging(true);
      }
    };

    const handleDragLeave = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);
    };

    const handleDragOver = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
    };

    const validateAndProcessFiles = (fileList: FileList | File[]): File[] => {
      let selectedFiles = Array.from(fileList);

      // Filter by max size
      if (maxSize) {
        selectedFiles = selectedFiles.filter((file) => {
          if (file.size > maxSize) {
            return false;
          }
          return true;
        });
      }

      // Limit number of files
      if (maxFiles && selectedFiles.length + files.length > maxFiles) {
        selectedFiles = selectedFiles.slice(0, maxFiles - files.length);
      }

      return selectedFiles;
    };

    const handleDrop = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);

      if (disabled) return;

      const droppedFiles = e.dataTransfer.files;
      if (droppedFiles.length > 0) {
        const validFiles = validateAndProcessFiles(droppedFiles);
        if (validFiles.length > 0) {
          onChange?.([...files, ...validFiles]);
        }
      }
    };

    const handleClick = () => {
      inputRef.current?.click();
    };

    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      if (e.target.files && e.target.files.length > 0) {
        const validFiles = validateAndProcessFiles(e.target.files);
        if (validFiles.length > 0) {
          onChange?.([...files, ...validFiles]);
        }
      }
      // Reset input
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
          onChange={handleInputChange}
          style={{ display: 'none' }}
          disabled={disabled}
        />

        <Paper
          onDragEnter={handleDragEnter}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onDrop={handleDrop}
          onClick={disabled ? undefined : handleClick}
          sx={{
            height,
            border: 2,
            borderStyle: 'dashed',
            borderColor: error ? 'error.main' : isDragging ? 'primary.main' : 'grey.300',
            bgcolor: isDragging ? 'primary.50' : 'grey.50',
            display: 'flex',
            flexDirection: 'column',
            alignItems: 'center',
            justifyContent: 'center',
            cursor: disabled ? 'not-allowed' : 'pointer',
            transition: 'all 0.2s',
            '&:hover': {
              bgcolor: disabled ? 'grey.50' : isDragging ? 'primary.50' : 'grey.100',
              borderColor: disabled ? 'grey.300' : 'primary.main',
            },
          }}
          elevation={0}
        >
          <CloudUpload
            sx={{
              fontSize: 64,
              color: isDragging ? 'primary.main' : 'grey.400',
              mb: 2,
            }}
          />
          <Typography variant="h6" color="text.primary" gutterBottom>
            {isDragging ? 'Drop files here' : 'Drag & drop files here'}
          </Typography>
          <Typography variant="body2" color="text.secondary">
            or click to browse
          </Typography>
          {helperText && !error && (
            <Typography variant="caption" color="text.secondary" sx={{ mt: 1 }}>
              {helperText}
            </Typography>
          )}
        </Paper>

        {error && errorMessage && (
          <Typography variant="caption" color="error" display="block" sx={{ mt: 0.5 }}>
            {errorMessage}
          </Typography>
        )}

        {files.length > 0 && (
          <List sx={{ mt: 2 }}>
            {files.map((file, index) => (
              <ListItem
                key={`${file.name}-${file.size}-${file.lastModified}`}
                secondaryAction={
                  <IconButton edge="end" onClick={() => handleRemove(index)} disabled={disabled}>
                    <Close />
                  </IconButton>
                }
                sx={{
                  bgcolor: 'background.paper',
                  mb: 1,
                  borderRadius: 1,
                  border: 1,
                  borderColor: 'grey.200',
                }}
              >
                <ListItemIcon>
                  <InsertDriveFile />
                </ListItemIcon>
                <ListItemText primary={file.name} secondary={formatFileSize(file.size)} />
              </ListItem>
            ))}
          </List>
        )}
      </Box>
    );
  },
);

DragDropUpload.displayName = 'DragDropUpload';

export default DragDropUpload;
