import axios, { type AxiosError, type AxiosInstance, type InternalAxiosRequestConfig } from 'axios';
import { ApiError } from './ApiError';
import type { CreateHttpClientOptions, HttpClient, HttpRequestConfig } from './types';
import { unwrapEnvelope } from './unwrapEnvelope';

const DEFAULT_TIMEOUT = 30_000;
const DEFAULT_DEVICE_TOKEN_HEADER = 'X-Device-Token';

function normalizeToken(token: string): string {
  const bearer = token.startsWith('Bearer ') ? token : `Bearer ${token}`;
  return bearer.replaceAll('"', '');
}

function mapAxiosError(error: AxiosError): ApiError {
  const body = error.response?.data;

  if (body && typeof body === 'object' && 'message' in body && 'statusCode' in body) {
    return ApiError.fromErrorEnvelope(
      body as { statusCode: number; message: string; error: string; timestamp?: string },
      error,
    );
  }

  return new ApiError({
    message: error.message || 'An error occurred',
    statusCode: error.response?.status ?? 500,
    cause: error,
  });
}

async function request<T>(client: AxiosInstance, config: HttpRequestConfig): Promise<T> {
  const response = await client.request<unknown>(config);
  return unwrapEnvelope<T>(response.data);
}

export function createHttpClient(options: CreateHttpClientOptions): HttpClient {
  const {
    baseUrl,
    getAuthToken,
    getDeviceToken,
    deviceTokenHeader = DEFAULT_DEVICE_TOKEN_HEADER,
    onUnauthorized,
    timeout = DEFAULT_TIMEOUT,
    headers = { 'Content-Type': 'application/json' },
  } = options;

  const client = axios.create({
    baseURL: baseUrl,
    timeout,
    headers,
  });

  client.interceptors.request.use(
    (config: InternalAxiosRequestConfig) => {
      const token = getAuthToken?.();
      if (token && config.headers) {
        config.headers.Authorization = normalizeToken(token);
      }

      const deviceToken = getDeviceToken?.();
      if (deviceToken && config.headers) {
        config.headers[deviceTokenHeader] = normalizeToken(deviceToken);
      }

      if (import.meta.env.DEV) {
        // biome-ignore lint/suspicious/noConsole: dev-only request logging
        console.log('API Request:', {
          method: config.method?.toUpperCase(),
          url: config.url,
          data: config.data,
        });
      }

      return config;
    },
    (error: AxiosError) => Promise.reject(error),
  );

  client.interceptors.response.use(
    (response) => {
      if (import.meta.env.DEV) {
        // biome-ignore lint/suspicious/noConsole: dev-only response logging
        console.log('API Response:', {
          url: response.config.url,
          status: response.status,
          data: response.data,
        });
      }
      return response;
    },
    async (error: AxiosError) => {
      if (import.meta.env.DEV) {
        // biome-ignore lint/suspicious/noConsole: dev-only error logging
        console.error('API Error:', {
          url: error.config?.url,
          status: error.response?.status,
          data: error.response?.data,
        });
      }

      if (error.response?.status === 401) {
        const body = error.response?.data;
        const message =
          body && typeof body === 'object' && 'message' in body
            ? String((body as { message: unknown }).message)
            : undefined;
        onUnauthorized?.({
          url: error.config?.url,
          message,
        });
      }

      return Promise.reject(mapAxiosError(error));
    },
  );

  const get = <T = unknown>(url: string, config?: HttpRequestConfig) =>
    request<T>(client, { method: 'GET', url, ...config });

  const post = <T = unknown>(url: string, data?: unknown, config?: HttpRequestConfig) =>
    request<T>(client, { method: 'POST', url, data, ...config });

  const put = <T = unknown>(url: string, data?: unknown, config?: HttpRequestConfig) =>
    request<T>(client, { method: 'PUT', url, data, ...config });

  const patch = <T = unknown>(url: string, data?: unknown, config?: HttpRequestConfig) =>
    request<T>(client, { method: 'PATCH', url, data, ...config });

  const del = <T = unknown>(url: string, config?: HttpRequestConfig) =>
    request<T>(client, { method: 'DELETE', url, ...config });

  const upload = async <T = unknown>(
    url: string,
    files: File | File[],
    additionalData?: Record<string, unknown>,
    onUploadProgress?: (progressEvent: unknown) => void,
  ): Promise<T> => {
    const formData = new FormData();

    if (Array.isArray(files)) {
      files.forEach((file, index) => {
        formData.append(`file${index}`, file);
      });
    } else {
      formData.append('file', files);
    }

    if (additionalData) {
      for (const [key, value] of Object.entries(additionalData)) {
        formData.append(key, JSON.stringify(value));
      }
    }

    return request<T>(client, {
      method: 'POST',
      url,
      data: formData,
      headers: { 'Content-Type': 'multipart/form-data' },
      ...(onUploadProgress ? { onUploadProgress } : {}),
    });
  };

  const download = async (
    url: string,
    filename?: string,
    config?: HttpRequestConfig,
  ): Promise<void> => {
    const response = await client.get(url, {
      responseType: 'blob',
      ...config,
    });

    const urlBlob = window.URL.createObjectURL(new Blob([response.data]));
    const link = document.createElement('a');
    link.href = urlBlob;
    link.setAttribute('download', filename || 'download');
    document.body.appendChild(link);
    link.click();
    link.remove();
    window.URL.revokeObjectURL(urlBlob);
  };

  return {
    get,
    post,
    put,
    patch,
    delete: del,
    upload,
    download,
    instance: client,
  };
}
