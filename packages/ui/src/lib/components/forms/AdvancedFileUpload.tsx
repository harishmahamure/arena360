'use client';

import {
  CheckCircle,
  Close,
  CloudUpload,
  Description,
  Error as ErrorIcon,
  Image as ImageIcon,
  InsertDriveFile,
  PictureAsPdf,
  VideoFile,
} from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Chip,
  IconButton,
  LinearProgress,
  List,
  ListItem,
  ListItemIcon,
  ListItemSecondaryAction,
  ListItemText,
  Paper,
  Typography,
} from '@mui/material';
import { forwardRef, useRef, useState } from 'react';

export interface FileUploadItem {
  file: File;
  id: string;
  progress: number;
  status: 'pending' | 'uploading' | 'completed' | 'error';
  error?: string;
  uploadedId?: string;
}

export interface AdvancedFileUploadProps {
  /**
   * Accept attribute for file input
   */
  accept?: string;
  /**
   * If true, allows multiple file selection
   */
  multiple?: boolean;
  /**
   * Maximum file size in MB
   */
  maxSizeMB?: number;
  /**
   * Maximum number of files
   */
  maxFiles?: number;
  /**
   * Callback when files are selected
   */
  onFilesSelected?: (files: File[]) => void;
  /**
   * Callback to handle upload (with progress tracking)
   */
  onUpload?: (
    file: File,
    onProgress: (progress: number) => void,
  ) => Promise<{ id: string } | undefined>;
  /**
   * Callback when upload completes
   */
  onUploadComplete?: (fileId: string, file: File) => void;
  /**
   * Callback when file is removed
   */
  onRemove?: (fileId: string, file: File) => void;
  /**
   * If true, starts upload automatically
   */
  autoUpload?: boolean;
  /**
   * If true, shows drag and drop zone
   */
  showDropZone?: boolean;
  /**
   * Height of drop zone
   */
  dropZoneHeight?: number | string;
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
   * Button label
   */
  buttonLabel?: string;
  /**
   * Show upload progress
   */
  showProgress?: boolean;
}

const getFileIcon = (fileType: string) => {
  if (fileType.startsWith('image/')) return <ImageIcon />;
  if (fileType.startsWith('video/')) return <VideoFile />;
  if (fileType === 'application/pdf') return <PictureAsPdf />;
  if (fileType.includes('document') || fileType.includes('word') || fileType.includes('text'))
    return <Description />;
  return <InsertDriveFile />;
};

const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${Math.round((bytes / k ** i) * 100) / 100} ${sizes[i]}`;
};

/**
 * AdvancedFileUpload - A comprehensive file upload component
 */
const AdvancedFileUpload = forwardRef<HTMLDivElement, AdvancedFileUploadProps>(
  (
    {
      accept,
      multiple = false,
      maxSizeMB = 10,
      maxFiles,
      onFilesSelected,
      onUpload,
      onUploadComplete,
      onRemove,
      autoUpload = false,
      showDropZone = true,
      dropZoneHeight = 200,
      error = false,
      errorMessage,
      helperText,
      disabled = false,
      buttonLabel = 'Select Files',
      showProgress = true,
    },
    ref,
  ) => {
    const [files, setFiles] = useState<FileUploadItem[]>([]);
    const [isDragging, setIsDragging] = useState(false);
    const inputRef = useRef<HTMLInputElement>(null);

    const validateFile = (file: File): string | null => {
      const maxBytes = maxSizeMB * 1024 * 1024;
      if (file.size > maxBytes) {
        return `File exceeds ${maxSizeMB}MB`;
      }

      if (maxFiles && files.length >= maxFiles) {
        return `Maximum ${maxFiles} files allowed`;
      }

      return null;
    };

    const handleFilesChange = async (selectedFiles: FileList | File[]) => {
      const fileArray = Array.from(selectedFiles);
      const validFiles: FileUploadItem[] = [];

      for (const file of fileArray) {
        const validationError = validateFile(file);

        if (validationError) {
          validFiles.push({
            file,
            id: `${Date.now()}-${Math.random()}`,
            progress: 0,
            status: 'error',
            error: validationError,
          });
        } else {
          validFiles.push({
            file,
            id: `${Date.now()}-${Math.random()}`,
            progress: 0,
            status: 'pending',
          });
        }
      }

      setFiles((prev) => [...prev, ...validFiles]);
      onFilesSelected?.(fileArray);

      // Auto upload if enabled
      if (autoUpload && onUpload) {
        for (const fileItem of validFiles) {
          if (fileItem.status === 'pending') {
            await handleUploadFile(fileItem);
          }
        }
      }
    };

    const handleUploadFile = async (fileItem: FileUploadItem) => {
      if (!onUpload) return;

      // Update status to uploading
      setFiles((prev) =>
        prev.map((f) => (f.id === fileItem.id ? { ...f, status: 'uploading', progress: 0 } : f)),
      );

      try {
        const result = await onUpload(fileItem.file, (progress) => {
          setFiles((prev) => prev.map((f) => (f.id === fileItem.id ? { ...f, progress } : f)));
        });

        // Update status to completed
        setFiles((prev) =>
          prev.map((f) =>
            f.id === fileItem.id
              ? {
                  ...f,
                  status: 'completed',
                  progress: 100,
                  uploadedId: result?.id,
                }
              : f,
          ),
        );

        onUploadComplete?.(result?.id || fileItem.id, fileItem.file);
      } catch (err) {
        // Update status to error
        setFiles((prev) =>
          prev.map((f) =>
            f.id === fileItem.id
              ? {
                  ...f,
                  status: 'error',
                  error: err instanceof Error ? err.message : 'Upload failed',
                }
              : f,
          ),
        );
      }
    };

    const handleUploadAll = async () => {
      const pendingFiles = files.filter((f) => f.status === 'pending');
      for (const fileItem of pendingFiles) {
        await handleUploadFile(fileItem);
      }
    };

    const handleRemove = (fileItem: FileUploadItem) => {
      setFiles((prev) => prev.filter((f) => f.id !== fileItem.id));
      onRemove?.(fileItem.uploadedId || fileItem.id, fileItem.file);
    };

    const handleDragEnter = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      if (!disabled) setIsDragging(true);
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

    const handleDrop = (e: React.DragEvent) => {
      e.preventDefault();
      e.stopPropagation();
      setIsDragging(false);

      if (disabled) return;

      const droppedFiles = e.dataTransfer.files;
      if (droppedFiles.length > 0) {
        handleFilesChange(droppedFiles);
      }
    };

    const handleClick = () => {
      inputRef.current?.click();
    };

    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      if (e.target.files && e.target.files.length > 0) {
        handleFilesChange(e.target.files);
      }
      // Reset input
      if (inputRef.current) {
        inputRef.current.value = '';
      }
    };

    const hasPendingUploads = files.some((f) => f.status === 'pending');
    const isUploading = files.some((f) => f.status === 'uploading');

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

        {showDropZone && (
          <Paper
            onDragEnter={handleDragEnter}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
            onClick={disabled ? undefined : handleClick}
            sx={{
              height: dropZoneHeight,
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
              mb: 2,
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
        )}

        {!showDropZone && (
          <Box sx={{ mb: 2 }}>
            <Button
              variant="contained"
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
          </Box>
        )}

        {error && errorMessage && (
          <Alert severity="error" sx={{ mb: 2 }}>
            {errorMessage}
          </Alert>
        )}

        {files.length > 0 && (
          <Box>
            <Box
              sx={{
                display: 'flex',
                justifyContent: 'space-between',
                alignItems: 'center',
                mb: 1,
              }}
            >
              <Typography variant="subtitle2" color="text.secondary">
                {files.length} file(s)
              </Typography>
              {!autoUpload && hasPendingUploads && onUpload && (
                <Button
                  size="small"
                  variant="contained"
                  onClick={handleUploadAll}
                  disabled={isUploading}
                >
                  Upload All
                </Button>
              )}
            </Box>

            <List>
              {files.map((fileItem) => (
                <ListItem
                  key={fileItem.id}
                  sx={{
                    bgcolor: 'background.paper',
                    mb: 1,
                    borderRadius: 1,
                    border: 1,
                    borderColor:
                      fileItem.status === 'error'
                        ? 'error.main'
                        : fileItem.status === 'completed'
                          ? 'success.main'
                          : 'grey.200',
                    flexDirection: 'column',
                    alignItems: 'flex-start',
                  }}
                >
                  <Box
                    sx={{
                      display: 'flex',
                      alignItems: 'center',
                      width: '100%',
                    }}
                  >
                    <ListItemIcon>{getFileIcon(fileItem.file.type)}</ListItemIcon>
                    <ListItemText
                      primary={fileItem.file.name}
                      secondary={formatFileSize(fileItem.file.size)}
                    />
                    <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
                      {fileItem.status === 'completed' && (
                        <Chip
                          icon={<CheckCircle />}
                          label="Uploaded"
                          size="small"
                          color="success"
                        />
                      )}
                      {fileItem.status === 'error' && (
                        <Chip icon={<ErrorIcon />} label="Error" size="small" color="error" />
                      )}
                      {fileItem.status === 'uploading' && (
                        <Chip label={`${fileItem.progress}%`} size="small" color="primary" />
                      )}
                      {!autoUpload && fileItem.status === 'pending' && onUpload && (
                        <Button
                          size="small"
                          onClick={() => handleUploadFile(fileItem)}
                          disabled={isUploading}
                        >
                          Upload
                        </Button>
                      )}
                      <ListItemSecondaryAction>
                        <IconButton
                          edge="end"
                          onClick={() => handleRemove(fileItem)}
                          disabled={fileItem.status === 'uploading'}
                          size="small"
                        >
                          <Close />
                        </IconButton>
                      </ListItemSecondaryAction>
                    </Box>
                  </Box>

                  {showProgress && fileItem.status === 'uploading' && fileItem.progress > 0 && (
                    <Box sx={{ width: '100%', mt: 1 }}>
                      <LinearProgress variant="determinate" value={fileItem.progress} />
                    </Box>
                  )}

                  {fileItem.status === 'error' && fileItem.error && (
                    <Typography variant="caption" color="error" sx={{ mt: 1, width: '100%' }}>
                      {fileItem.error}
                    </Typography>
                  )}
                </ListItem>
              ))}
            </List>
          </Box>
        )}
      </Box>
    );
  },
);

AdvancedFileUpload.displayName = 'AdvancedFileUpload';

export default AdvancedFileUpload;
