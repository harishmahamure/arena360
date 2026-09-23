export interface SuccessEnvelope<T> {
  success: boolean;
  statusCode: number;
  timestamp: string;
  data: T;
}

export interface ErrorEnvelope {
  statusCode: number;
  message: string;
  error: string;
  timestamp: string;
}

export interface CreateHttpClientOptions {
  baseUrl: string;
  getAuthToken?: () => string | null | undefined;
  getDeviceToken?: () => string | null | undefined;
  deviceTokenHeader?: string;
  onUnauthorized?: (context: { url?: string; message?: string; authHeader?: string }) => void;
  timeout?: number;
  headers?: Record<string, string>;
}

export interface HttpRequestConfig {
  params?: object;
  headers?: Record<string, string>;
  timeout?: number;
  responseType?: 'json' | 'blob' | 'arraybuffer';
}

export interface HttpClient {
  get: <T = unknown>(url: string, config?: HttpRequestConfig) => Promise<T>;
  post: <T = unknown>(url: string, data?: unknown, config?: HttpRequestConfig) => Promise<T>;
  put: <T = unknown>(url: string, data?: unknown, config?: HttpRequestConfig) => Promise<T>;
  patch: <T = unknown>(url: string, data?: unknown, config?: HttpRequestConfig) => Promise<T>;
  delete: <T = unknown>(url: string, config?: HttpRequestConfig) => Promise<T>;
  upload: <T = unknown>(
    url: string,
    files: File | File[],
    additionalData?: Record<string, unknown>,
    onUploadProgress?: (progressEvent: unknown) => void,
  ) => Promise<T>;
  download: (url: string, filename?: string, config?: HttpRequestConfig) => Promise<void>;
}
