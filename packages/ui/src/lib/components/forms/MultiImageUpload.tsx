'use client';

import {
  CheckCircle,
  CloudUpload,
  Delete,
  Error as ErrorIcon,
  Image as ImageIcon,
} from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardActions,
  CardMedia,
  Chip,
  GridLegacy as Grid,
  IconButton,
  LinearProgress,
  Paper,
  Typography,
} from '@mui/material';
import { forwardRef, useRef, useState } from 'react';

export interface ImageUploadItem {
  file: File;
  id: string;
  preview: string;
  progress: number;
  status: 'pending' | 'uploading' | 'completed' | 'error';
  error?: string;
  uploadedId?: string;
}

export interface MultiImageUploadProps {
  /**
   * Maximum file size in MB
   */
  maxSizeMB?: number;
  /**
   * Maximum number of images
   */
  maxImages?: number;
  /**
   * Callback when images are selected
   */
  onImagesSelected?: (files: File[]) => void;
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
  onUploadComplete?: (imageId: string, file: File) => void;
  /**
   * Callback when image is removed
   */
  onRemove?: (imageId: string, file: File) => void;
  /**
   * If true, starts upload automatically
   */
  autoUpload?: boolean;
  /**
   * Width of preview image
   */
  previewWidth?: number | string;
  /**
   * Height of preview image
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
   * Grid columns for preview
   */
  columns?: number;
}

/**
 * MultiImageUpload - Upload multiple images with preview
 */
const MultiImageUpload = forwardRef<HTMLDivElement, MultiImageUploadProps>(
  (
    {
      maxSizeMB = 10,
      maxImages,
      onImagesSelected,
      onUpload,
      onUploadComplete,
      onRemove,
      autoUpload = false,
      previewHeight = 200,
      error = false,
      errorMessage,
      helperText,
      disabled = false,
      columns = 3,
    },
    ref,
  ) => {
    const [images, setImages] = useState<ImageUploadItem[]>([]);
    const inputRef = useRef<HTMLInputElement>(null);

    const validateImage = (file: File): string | null => {
      // Check if it's an image
      if (!file.type.startsWith('image/')) {
        return 'File must be an image';
      }

      // Check file size
      const maxBytes = maxSizeMB * 1024 * 1024;
      if (file.size > maxBytes) {
        return `Image exceeds ${maxSizeMB}MB`;
      }

      // Check max images
      if (maxImages && images.length >= maxImages) {
        return `Maximum ${maxImages} images allowed`;
      }

      return null;
    };

    const handleImagesChange = async (selectedFiles: FileList | File[]) => {
      const fileArray = Array.from(selectedFiles);
      const validImages: ImageUploadItem[] = [];

      for (const file of fileArray) {
        const validationError = validateImage(file);

        if (validationError) {
          validImages.push({
            file,
            id: `${Date.now()}-${Math.random()}`,
            preview: '',
            progress: 0,
            status: 'error',
            error: validationError,
          });
        } else {
          // Create preview URL
          const preview = URL.createObjectURL(file);
          validImages.push({
            file,
            id: `${Date.now()}-${Math.random()}`,
            preview,
            progress: 0,
            status: 'pending',
          });
        }
      }

      setImages((prev) => [...prev, ...validImages]);
      onImagesSelected?.(fileArray);

      // Auto upload if enabled
      if (autoUpload && onUpload) {
        for (const imageItem of validImages) {
          if (imageItem.status === 'pending') {
            await handleUploadImage(imageItem);
          }
        }
      }
    };

    const handleUploadImage = async (imageItem: ImageUploadItem) => {
      if (!onUpload) return;

      // Update status to uploading
      setImages((prev) =>
        prev.map((img) =>
          img.id === imageItem.id ? { ...img, status: 'uploading', progress: 0 } : img,
        ),
      );

      try {
        const result = await onUpload(imageItem.file, (progress) => {
          setImages((prev) =>
            prev.map((img) => (img.id === imageItem.id ? { ...img, progress } : img)),
          );
        });

        // Update status to completed
        setImages((prev) =>
          prev.map((img) =>
            img.id === imageItem.id
              ? {
                  ...img,
                  status: 'completed',
                  progress: 100,
                  uploadedId: result?.id,
                }
              : img,
          ),
        );

        onUploadComplete?.(result?.id || imageItem.id, imageItem.file);
      } catch (err) {
        // Update status to error
        setImages((prev) =>
          prev.map((img) =>
            img.id === imageItem.id
              ? {
                  ...img,
                  status: 'error',
                  error: err instanceof Error ? err.message : 'Upload failed',
                }
              : img,
          ),
        );
      }
    };

    const handleUploadAll = async () => {
      const pendingImages = images.filter((img) => img.status === 'pending');
      for (const imageItem of pendingImages) {
        await handleUploadImage(imageItem);
      }
    };

    const handleRemove = (imageItem: ImageUploadItem) => {
      // Revoke preview URL to free memory
      if (imageItem.preview) {
        URL.revokeObjectURL(imageItem.preview);
      }

      setImages((prev) => prev.filter((img) => img.id !== imageItem.id));
      onRemove?.(imageItem.uploadedId || imageItem.id, imageItem.file);
    };

    const handleClick = () => {
      inputRef.current?.click();
    };

    const handleInputChange = (e: React.ChangeEvent<HTMLInputElement>) => {
      if (e.target.files && e.target.files.length > 0) {
        handleImagesChange(e.target.files);
      }
      // Reset input
      if (inputRef.current) {
        inputRef.current.value = '';
      }
    };

    const hasPendingUploads = images.some((img) => img.status === 'pending');
    const isUploading = images.some((img) => img.status === 'uploading');

    return (
      <Box ref={ref}>
        <input
          ref={inputRef}
          type="file"
          accept="image/*"
          multiple
          onChange={handleInputChange}
          style={{ display: 'none' }}
          disabled={disabled}
        />

        <Box sx={{ mb: 2, display: 'flex', gap: 2, alignItems: 'center' }}>
          <Button
            variant="contained"
            startIcon={<CloudUpload />}
            onClick={handleClick}
            disabled={disabled || (maxImages ? images.length >= maxImages : false)}
            color={error ? 'error' : 'primary'}
          >
            Select Images
          </Button>

          {!autoUpload && hasPendingUploads && onUpload && (
            <Button variant="outlined" onClick={handleUploadAll} disabled={isUploading}>
              Upload All
            </Button>
          )}

          {images.length > 0 && (
            <Typography variant="body2" color="text.secondary">
              {images.length} image(s)
              {maxImages && ` / ${maxImages}`}
            </Typography>
          )}
        </Box>

        {helperText && !error && (
          <Typography variant="caption" color="text.secondary" display="block" sx={{ mb: 2 }}>
            {helperText}
          </Typography>
        )}

        {error && errorMessage && (
          <Alert severity="error" sx={{ mb: 2 }}>
            {errorMessage}
          </Alert>
        )}

        {images.length === 0 ? (
          <Paper
            onClick={disabled ? undefined : handleClick}
            sx={{
              height: 300,
              border: 2,
              borderStyle: 'dashed',
              borderColor: error ? 'error.main' : 'grey.300',
              bgcolor: 'grey.50',
              display: 'flex',
              flexDirection: 'column',
              alignItems: 'center',
              justifyContent: 'center',
              cursor: disabled ? 'not-allowed' : 'pointer',
              transition: 'all 0.2s',
              '&:hover': {
                bgcolor: disabled ? 'grey.50' : 'grey.100',
                borderColor: disabled ? 'grey.300' : 'primary.main',
              },
            }}
            elevation={0}
          >
            <ImageIcon sx={{ fontSize: 64, color: 'grey.400', mb: 2 }} />
            <Typography variant="h6" color="text.primary" gutterBottom>
              No images selected
            </Typography>
            <Typography variant="body2" color="text.secondary">
              Click to select images
            </Typography>
          </Paper>
        ) : (
          <Grid container spacing={2}>
            {images.map((imageItem) => (
              <Grid item xs={12} sm={6} md={12 / columns} key={imageItem.id}>
                <Card
                  sx={{
                    position: 'relative',
                    border: 1,
                    borderColor:
                      imageItem.status === 'error'
                        ? 'error.main'
                        : imageItem.status === 'completed'
                          ? 'success.main'
                          : 'grey.200',
                  }}
                >
                  {imageItem.preview ? (
                    <CardMedia
                      component="img"
                      height={previewHeight}
                      image={imageItem.preview}
                      alt={imageItem.file.name}
                      sx={{ objectFit: 'cover' }}
                    />
                  ) : (
                    <Box
                      sx={{
                        height: previewHeight,
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        bgcolor: 'grey.100',
                      }}
                    >
                      <ImageIcon sx={{ fontSize: 48, color: 'grey.400' }} />
                    </Box>
                  )}

                  {/* Status overlay */}
                  <Box
                    sx={{
                      position: 'absolute',
                      top: 8,
                      right: 8,
                    }}
                  >
                    {imageItem.status === 'completed' && (
                      <Chip
                        icon={<CheckCircle />}
                        label="Uploaded"
                        size="small"
                        color="success"
                        sx={{ bgcolor: 'success.main' }}
                      />
                    )}
                    {imageItem.status === 'error' && (
                      <Chip
                        icon={<ErrorIcon />}
                        label="Error"
                        size="small"
                        color="error"
                        sx={{ bgcolor: 'error.main' }}
                      />
                    )}
                    {imageItem.status === 'uploading' && (
                      <Chip
                        label={`${imageItem.progress}%`}
                        size="small"
                        color="primary"
                        sx={{ bgcolor: 'primary.main', color: 'white' }}
                      />
                    )}
                  </Box>

                  {/* Progress bar */}
                  {imageItem.status === 'uploading' && imageItem.progress > 0 && (
                    <Box
                      sx={{
                        position: 'absolute',
                        bottom: 0,
                        left: 0,
                        right: 0,
                      }}
                    >
                      <LinearProgress variant="determinate" value={imageItem.progress} />
                    </Box>
                  )}

                  <CardActions
                    sx={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      px: 1,
                      py: 0.5,
                    }}
                  >
                    <Typography
                      variant="caption"
                      noWrap
                      sx={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis' }}
                    >
                      {imageItem.file.name}
                    </Typography>

                    <Box sx={{ display: 'flex', gap: 0.5 }}>
                      {!autoUpload && imageItem.status === 'pending' && onUpload && (
                        <Button
                          size="small"
                          onClick={() => handleUploadImage(imageItem)}
                          disabled={isUploading}
                        >
                          Upload
                        </Button>
                      )}
                      <IconButton
                        size="small"
                        onClick={() => handleRemove(imageItem)}
                        disabled={imageItem.status === 'uploading'}
                        color="error"
                      >
                        <Delete fontSize="small" />
                      </IconButton>
                    </Box>
                  </CardActions>

                  {imageItem.status === 'error' && imageItem.error && (
                    <Box sx={{ px: 1, pb: 1 }}>
                      <Typography variant="caption" color="error">
                        {imageItem.error}
                      </Typography>
                    </Box>
                  )}
                </Card>
              </Grid>
            ))}
          </Grid>
        )}
      </Box>
    );
  },
);

MultiImageUpload.displayName = 'MultiImageUpload';

export default MultiImageUpload;
