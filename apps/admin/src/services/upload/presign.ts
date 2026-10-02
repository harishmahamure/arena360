import { http } from '@gaming-cafe/utils';

interface PresignResponse {
  uploadUrl: string;
  publicUrl: string;
  key: string;
}

/**
 * Request a presigned PUT URL (DRAFT-0022), upload the file directly to object
 * storage, and return the stable public URL to persist on the record.
 */
export const uploadAsset = async (file: File): Promise<string> => {
  const presigned = await http.post<PresignResponse>('/uploads/presign', {
    fileName: file.name,
    contentType: file.type || 'application/octet-stream',
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
