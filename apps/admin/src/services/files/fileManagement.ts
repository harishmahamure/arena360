import { http } from '@gaming-cafe/utils';
import type {
  CreateFileDto,
  FileCategory,
  FileFilterDto,
  FileRecord,
  FileWithDownloadUrl,
  PresignedUploadUrlResponse,
  StorageStats,
  UpdateFileDto,
} from './types';

const BASE_URL = '/files';
const STORAGE_URL = '/storage';

export const getPresignedUploadUrl = async (params: {
  key: string;
  contentType: string;
  bucket?: string;
}): Promise<PresignedUploadUrlResponse> => {
  return http.post<PresignedUploadUrlResponse>(`${STORAGE_URL}/upload-url`, params);
};

export const uploadToR2 = async (
  presignedUrl: string,
  file: File,
  onProgress?: (progress: number) => void,
): Promise<void> => {
  return new Promise((resolve, reject) => {
    const xhr = new XMLHttpRequest();

    xhr.upload.addEventListener('progress', (e) => {
      if (e.lengthComputable && onProgress) {
        const percentage = Math.round((e.loaded / e.total) * 100);
        onProgress(percentage);
      }
    });

    xhr.addEventListener('load', () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        resolve();
      } else {
        reject(new Error(`Upload failed with status ${xhr.status}`));
      }
    });

    xhr.addEventListener('error', () => {
      reject(new Error('Upload failed'));
    });

    xhr.addEventListener('abort', () => {
      reject(new Error('Upload aborted'));
    });

    xhr.open('PUT', presignedUrl);
    xhr.setRequestHeader('Content-Type', file.type);
    xhr.send(file);
  });
};

export const createFileRecord = async (data: CreateFileDto): Promise<FileRecord> => {
  return http.post<FileRecord>(BASE_URL, data);
};

export const listFiles = async (
  filters?: FileFilterDto,
): Promise<{
  data: FileRecord[];
  total: number;
  page: number;
  limit: number;
}> => {
  const params = new URLSearchParams();

  if (filters) {
    Object.entries(filters).forEach(([key, value]) => {
      if (value !== undefined && value !== null) {
        params.append(key, String(value));
      }
    });
  }

  return http.get<{
    data: FileRecord[];
    total: number;
    page: number;
    limit: number;
  }>(`${BASE_URL}?${params.toString()}`);
};

export const getFileById = async (id: string): Promise<FileRecord> => {
  return http.get<FileRecord>(`${BASE_URL}/${id}`);
};

export const getFileWithDownloadUrl = async (id: string): Promise<FileWithDownloadUrl> => {
  return http.get<FileWithDownloadUrl>(`${BASE_URL}/${id}/download-url`);
};

/**
 * Update file metadata
 */
export const updateFile = async (id: string, data: UpdateFileDto): Promise<FileRecord> => {
  return http.put<FileRecord>(`${BASE_URL}/${id}`, data);
};

/**
 * Delete file (soft delete)
 */
export const deleteFile = async (id: string): Promise<void> => {
  await http.delete(`${BASE_URL}/${id}`);
};

/**
 * Bulk delete files (sequential deletes; no bulk endpoint on backend)
 */
export const bulkDeleteFiles = async (ids: string[]): Promise<void> => {
  await Promise.all(ids.map((id) => deleteFile(id)));
};

/**
 * Archive file
 */
export const archiveFile = async (id: string): Promise<FileRecord> => {
  return http.put<FileRecord>(`${BASE_URL}/${id}/archive`, {});
};

/**
 * Activate file
 */
export const activateFile = async (id: string): Promise<FileRecord> => {
  return http.put<FileRecord>(`${BASE_URL}/${id}/activate`, {});
};

/**
 * Restore deleted file (maps to activate on backend)
 */
export const restoreFile = async (id: string): Promise<FileRecord> => {
  return http.put<FileRecord>(`${BASE_URL}/${id}/activate`, {});
};

/**
 * Get files by category
 */
export const getFilesByCategory = async (category: FileCategory): Promise<FileRecord[]> => {
  const result = await listFiles({ category, limit: 100 });
  return result.data;
};

/**
 * Get files by uploader
 */
export const getFilesByUploader = async (uploaderId: string): Promise<FileRecord[]> => {
  const result = await listFiles({ uploadedBy: uploaderId, limit: 100 });
  return result.data;
};

/**
 * Get files by related entity
 */
export const getFilesByRelatedEntity = async (
  entityType: string,
  entityId: string,
): Promise<FileRecord[]> => {
  const result = await listFiles({
    relatedEntityType: entityType,
    relatedEntityId: entityId,
    limit: 100,
  });
  return result.data;
};

/**
 * Get storage statistics
 */
export const getStorageStats = async (): Promise<StorageStats> => {
  return http.get<StorageStats>(`${BASE_URL}/stats`);
};

/**
 * Cleanup expired files (not supported by backend; returns 0)
 */
export const cleanupExpiredFiles = async (): Promise<number> => {
  return 0;
};

/**
 * Complete upload workflow: Get URL, upload to R2, create record
 */
export const completeFileUpload = async (
  file: File,
  options: {
    category: FileCategory;
    visibility?: 'public' | 'private' | 'admin';
    description?: string;
    tags?: string[];
    relatedEntityType?: string;
    relatedEntityId?: string;
    metadata?: Record<string, unknown>;
    storageKeyPrefix?: string;
  },
  onProgress?: (progress: number) => void,
): Promise<FileRecord> => {
  const timestamp = Date.now();
  const sanitizedName = file.name.replace(/[^a-zA-Z0-9.-]/g, '_');
  const keyPrefix = options.storageKeyPrefix || options.category;
  const storageKey = `${keyPrefix}/${timestamp}-${sanitizedName}`;

  const { url, key, bucket } = await getPresignedUploadUrl({
    key: storageKey,
    contentType: file.type,
  });

  await uploadToR2(url, file, onProgress);

  // Step 4: Get file extension
  const extension = file.name.split('.').pop() || '';

  // Step 5: Get image dimensions if it's an image
  let width: number | undefined;
  let height: number | undefined;

  if (file.type.startsWith('image/')) {
    try {
      const dimensions = await getImageDimensions(file);
      width = dimensions.width;
      height = dimensions.height;
    } catch (_error) {}
  }

  // Step 6: Create file record in database
  const fileRecord = await createFileRecord({
    fileName: sanitizedName,
    originalFileName: file.name,
    storageKey: key,
    bucket,
    contentType: file.type,
    fileSize: file.size,
    extension,
    category: options.category,
    visibility: options.visibility,
    description: options.description,
    tags: options.tags,
    metadata: options.metadata,
    relatedEntityType: options.relatedEntityType,
    relatedEntityId: options.relatedEntityId,
    width,
    height,
  });

  return fileRecord;
};

/**
 * Helper: Get image dimensions
 */
const getImageDimensions = (file: File): Promise<{ width: number; height: number }> => {
  return new Promise((resolve, reject) => {
    const img = new Image();
    const url = URL.createObjectURL(file);

    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve({
        width: img.naturalWidth,
        height: img.naturalHeight,
      });
    };

    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error('Failed to load image'));
    };

    img.src = url;
  });
};

/**
 * Helper: Format file size to human readable
 */
export const formatFileSize = (bytes: number): string => {
  if (bytes === 0) return '0 Bytes';
  const k = 1024;
  const sizes = ['Bytes', 'KB', 'MB', 'GB', 'TB'];
  const i = Math.floor(Math.log(bytes) / Math.log(k));
  return `${Math.round((bytes / k ** i) * 100) / 100} ${sizes[i]}`;
};

/**
 * Helper: Validate file size
 */
export const validateFileSize = (
  file: File,
  maxSizeMB: number,
): { valid: boolean; error?: string } => {
  const maxBytes = maxSizeMB * 1024 * 1024;
  if (file.size > maxBytes) {
    return {
      valid: false,
      error: `File size exceeds maximum of ${maxSizeMB}MB`,
    };
  }
  return { valid: true };
};

/**
 * Helper: Validate file type
 */
export const validateFileType = (
  file: File,
  allowedTypes: string[],
): { valid: boolean; error?: string } => {
  const fileType = file.type;
  const fileExtension = `.${file.name.split('.').pop()?.toLowerCase()}`;

  const isValid = allowedTypes.some((type) => {
    if (type.includes('*')) {
      // Handle wildcard types like "image/*"
      const baseType = type.split('/')[0];
      return fileType.startsWith(`${baseType}/`);
    }
    // Handle exact types or extensions
    return fileType === type || fileExtension === type;
  });

  if (!isValid) {
    return {
      valid: false,
      error: `File type not allowed. Allowed types: ${allowedTypes.join(', ')}`,
    };
  }

  return { valid: true };
};
