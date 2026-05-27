/**
 * File Management Types
 */

export type FileCategory =
  | 'profile'
  | 'product'
  | 'game'
  | 'document'
  | 'receipt'
  | 'invoice'
  | 'temp'
  | 'other';

export type FileStatus = 'uploading' | 'active' | 'archived' | 'deleted';

export type FileVisibility = 'public' | 'private' | 'admin';

export type StorageType = 'r2' | 's3' | 'local';

export interface FileMetadata {
  [key: string]: any;
}

export interface FileRecord {
  id: string;
  fileName: string;
  originalFileName: string;
  storageKey: string;
  bucket: string;
  contentType: string;
  fileSize: number;
  extension: string;
  category: FileCategory;
  status: FileStatus;
  visibility: FileVisibility;
  storageType: StorageType;
  width?: number;
  height?: number;
  duration?: number;
  etag?: string;
  metadata?: FileMetadata;
  description?: string;
  tags?: string[];
  uploadedBy?: string;
  relatedEntityType?: string;
  relatedEntityId?: string;
  downloadCount: number;
  lastAccessedAt?: Date;
  expiresAt?: Date;
  deletedAt?: Date;
  createdAt: Date;
  updatedAt: Date;
}

export interface CreateFileDto {
  fileName: string;
  originalFileName: string;
  storageKey: string;
  bucket?: string;
  contentType: string;
  fileSize: number;
  extension: string;
  category: FileCategory;
  visibility?: FileVisibility;
  storageType?: StorageType;
  width?: number;
  height?: number;
  duration?: number;
  etag?: string;
  metadata?: FileMetadata;
  description?: string;
  tags?: string[];
  relatedEntityType?: string;
  relatedEntityId?: string;
  expiresAt?: Date;
}

export interface UpdateFileDto {
  fileName?: string;
  description?: string;
  tags?: string[];
  metadata?: FileMetadata;
  category?: FileCategory;
  visibility?: FileVisibility;
  status?: FileStatus;
}

export interface FileFilterDto {
  category?: FileCategory;
  status?: FileStatus;
  contentType?: string;
  uploadedBy?: string;
  relatedEntityType?: string;
  relatedEntityId?: string;
  page?: number;
  limit?: number;
}

export interface FileWithDownloadUrl extends FileRecord {
  downloadUrl: string;
}

export interface StorageStats {
  totalFiles: number;
  totalSize: number;
  byCategory: {
    [key in FileCategory]?: {
      count: number;
      size: number;
    };
  };
  byStatus: {
    [key in FileStatus]?: number;
  };
}

export interface PresignedUploadUrlResponse {
  url: string;
  key: string;
  bucket: string;
}

export interface UploadProgress {
  loaded: number;
  total: number;
  percentage: number;
}
