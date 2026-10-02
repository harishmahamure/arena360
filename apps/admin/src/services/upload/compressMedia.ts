export interface CompressOptions {
  /** Longest edge in pixels; larger media is scaled down, smaller media is never upscaled. */
  maxDimension?: number;
  /** WebP quality between 0 and 1. */
  quality?: number;
  /** Target WebM video bitrate in bits per second. */
  videoBitsPerSecond?: number;
}

const PASSTHROUGH_IMAGES = new Set(['image/svg+xml', 'image/gif']);

function renamed(name: string, extension: string): string {
  const base = name.replace(/\.[^./\\]+$/, '') || 'upload';
  return `${base}.${extension}`;
}

export function scaledSize(width: number, height: number, maxDimension: number) {
  const scale = Math.min(1, maxDimension / Math.max(width, height, 1));
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

/**
 * Re-encode an image as WebP, scaled to `maxDimension`. SVG and GIF are kept as-is
 * (vector / animation would be lost), and the original wins if encoding is unsupported
 * or would not make the file smaller.
 */
export async function compressImage(file: File, options: CompressOptions = {}): Promise<File> {
  const { maxDimension = 1920, quality = 0.82 } = options;
  if (PASSTHROUGH_IMAGES.has(file.type) || typeof createImageBitmap !== 'function') return file;
  const bitmap = await createImageBitmap(file);
  try {
    const { width, height } = scaledSize(bitmap.width, bitmap.height, maxDimension);
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext('2d');
    if (!context) return file;
    context.drawImage(bitmap, 0, 0, width, height);
    const blob = await new Promise<Blob | null>((resolve) =>
      canvas.toBlob(resolve, 'image/webp', quality),
    );
    const resized = width !== bitmap.width || height !== bitmap.height;
    if (!blob || blob.type !== 'image/webp') return file;
    if (file.type === 'image/webp' && !resized && blob.size >= file.size) return file;
    return new File([blob], renamed(file.name, 'webp'), { type: 'image/webp' });
  } finally {
    bitmap.close();
  }
}

function webmMimeType(): string | undefined {
  if (typeof MediaRecorder === 'undefined') return undefined;
  return ['video/webm;codecs=vp9,opus', 'video/webm;codecs=vp8,opus', 'video/webm'].find((type) =>
    MediaRecorder.isTypeSupported(type),
  );
}

type CapturableVideo = HTMLVideoElement & { captureStream?: () => MediaStream };

/**
 * Transcode a video to WebM in the browser by replaying it through a canvas into
 * MediaRecorder. This runs in real time, so a 30 s clip takes about 30 s. Browsers
 * without WebM recording (e.g. Safari) upload the original file.
 */
export async function compressVideo(file: File, options: CompressOptions = {}): Promise<File> {
  const { maxDimension = 1280, videoBitsPerSecond = 1_500_000 } = options;
  const mimeType = webmMimeType();
  if (file.type === 'video/webm' || !mimeType) return file;

  const url = URL.createObjectURL(file);
  const video = document.createElement('video') as CapturableVideo;
  video.src = url;
  video.muted = true;
  video.playsInline = true;
  try {
    await new Promise<void>((resolve, reject) => {
      video.onloadedmetadata = () => resolve();
      video.onerror = () => reject(new Error('Unsupported video'));
    });
    const { width, height } = scaledSize(video.videoWidth, video.videoHeight, maxDimension);
    const canvas = document.createElement('canvas');
    canvas.width = width;
    canvas.height = height;
    const context = canvas.getContext('2d');
    if (!context || typeof canvas.captureStream !== 'function') return file;
    const stream = canvas.captureStream(30);
    video
      .captureStream?.()
      .getAudioTracks()
      .forEach((track) => {
        stream.addTrack(track);
      });

    const chunks: Blob[] = [];
    const recorder = new MediaRecorder(stream, { mimeType, videoBitsPerSecond });
    recorder.ondataavailable = (event) => {
      if (event.data.size) chunks.push(event.data);
    };
    const stopped = new Promise<void>((resolve) => {
      recorder.onstop = () => resolve();
    });
    let frame = 0;
    const draw = () => {
      context.drawImage(video, 0, 0, width, height);
      if (!video.ended) frame = requestAnimationFrame(draw);
    };
    video.onended = () => {
      cancelAnimationFrame(frame);
      recorder.stop();
    };
    recorder.start(1000);
    await video.play();
    draw();
    await stopped;
    stream.getTracks().forEach((track) => {
      track.stop();
    });

    const blob = new Blob(chunks, { type: 'video/webm' });
    if (!blob.size || blob.size >= file.size) return file;
    return new File([blob], renamed(file.name, 'webm'), { type: 'video/webm' });
  } catch {
    return file;
  } finally {
    URL.revokeObjectURL(url);
  }
}

/** Compress media before upload: images become WebP, videos become WebM. Other files pass through. */
export async function compressMedia(file: File, options?: CompressOptions): Promise<File> {
  try {
    if (file.type.startsWith('image/')) return await compressImage(file, options);
    if (file.type.startsWith('video/')) return await compressVideo(file, options);
  } catch {
    /* Compression is best-effort; uploading the original is always safe. */
  }
  return file;
}
