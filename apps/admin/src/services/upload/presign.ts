import { http } from '@gaming-cafe/utils';
import { type CompressOptions, compressMedia } from './compressMedia';

interface PresignResponse {
  uploadUrl: string;
  publicUrl: string;
  key: string;
}

export interface UploadOptions extends CompressOptions {
  /** `avatar`: own profile photo (any panel user). `branding`: logos, requires settings:write. */
  purpose?: 'avatar' | 'branding';
}

/**
 * Compress media (WebP images, WebM video), request a presigned PUT URL (DRAFT-0022),
 * upload the bytes directly to object storage, and return the public URL to persist.
 */
export const uploadAsset = async (original: File, options: UploadOptions = {}): Promise<string> => {
  const { purpose, ...compression } = options;
  const file = await compressMedia(original, compression);
  const presigned = await http.post<PresignResponse>('/uploads/presign', {
    fileName: file.name,
    contentType: file.type || 'application/octet-stream',
    purpose,
  });

  const res = await fetch(presigned.uploadUrl, {
    method: 'PUT',
    body: file,
    headers: file.type ? { 'Content-Type': file.type } : undefined,
  });
  if (!res.ok) {
    throw new Error(`Upload failed: HTTP ${res.status}`);
  }
  return presigned.publicUrl;
};
