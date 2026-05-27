/**
 * useFileUpload Hook
 * Custom hook for handling file uploads with progress tracking
 */

import { toastUtils } from '@gaming-cafe/utils';
import { useCallback } from 'react';
import { completeFileUpload } from '../services/files/fileManagement';
import type { FileCategory, FileRecord } from '../services/files/types';

const showToast = toastUtils;

export interface UploadOptions {
  category: FileCategory;
  visibility?: 'public' | 'private' | 'admin';
  description?: string;
  tags?: string[];
  relatedEntityType?: string;
  relatedEntityId?: string;
  metadata?: Record<string, unknown>;
  storageKeyPrefix?: string;
  maxSizeMB?: number;
  allowedTypes?: string[];
}

export interface FileUploadState {
  file: File;
  progress: number;
  status: 'pending' | 'uploading' | 'completed' | 'error';
  error?: string;
  fileRecord?: FileRecord;
}

export interface UseFileUploadReturn {
  uploadFile: (file: File, options: UploadOptions) => Promise<FileRecord | null>;
  uploadFiles: (files: File[], options: UploadOptions) => Promise<FileRecord[]>;
}

export const useFileUpload = (): UseFileUploadReturn => {
  const validateFile = useCallback((file: File, options: UploadOptions): string | null => {
    if (options.maxSizeMB) {
      const maxBytes = options.maxSizeMB * 1024 * 1024;
      if (file.size > maxBytes) {
        return `File size exceeds maximum of ${options.maxSizeMB}MB`;
      }
    }

    if (options.allowedTypes && options.allowedTypes.length > 0) {
      const fileType = file.type;
      const fileExtension = `.${file.name.split('.').pop()?.toLowerCase()}`;

      const isValid = options.allowedTypes.some((type) => {
        if (type.includes('*')) {
          const baseType = type.split('/')[0];
          return fileType.startsWith(`${baseType}/`);
        }
        return fileType === type || fileExtension === type;
      });

      if (!isValid) {
        return `File type not allowed. Allowed: ${options.allowedTypes.join(', ')}`;
      }
    }

    return null;
  }, []);

  const uploadFile = useCallback(
    async (file: File, options: UploadOptions): Promise<FileRecord | null> => {
      const validationError = validateFile(file, options);
      if (validationError) {
        showToast.error(validationError);
        return null;
      }

      try {
        const fileRecord = await completeFileUpload(file, options, (_progress) => {});

        showToast.success(`File "${file.name}" uploaded successfully`);
        return fileRecord;
      } catch (error) {
        showToast.error(
          `Failed to upload "${file.name}": ${
            error instanceof Error ? error.message : 'Unknown error'
          }`,
        );
        return null;
      }
    },
    [validateFile],
  );

  /**
   * Upload multiple files
   */
  const uploadFiles = useCallback(
    async (files: File[], options: UploadOptions): Promise<FileRecord[]> => {
      const results: FileRecord[] = [];

      for (const file of files) {
        const result = await uploadFile(file, options);
        if (result) {
          results.push(result);
        }
      }

      return results;
    },
    [uploadFile],
  );

  return {
    uploadFile,
    uploadFiles,
  };
};
