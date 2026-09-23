import { createArena360GrpcWebTransport, encodeJson } from '@gaming-cafe/proto';
import { ApiError } from './ApiError';
import type { CreateHttpClientOptions, HttpClient, HttpRequestConfig } from './types';
import { unwrapEnvelope } from './unwrapEnvelope';

const DEFAULT_TIMEOUT = 30_000;
const DEFAULT_DEVICE_TOKEN_HEADER = 'X-Player-Token';
const decoder = new TextDecoder();

function normalizeToken(token: string): string {
  const bearer = token.startsWith('Bearer ') ? token : `Bearer ${token}`;
  return bearer.replaceAll('"', '');
}

function encodeQuery(params?: object): string {
  if (!params) return '';
  const query = new URLSearchParams();
  for (const [key, raw] of Object.entries(params)) {
    if (raw === undefined || raw === null) continue;
    const values = Array.isArray(raw) ? raw : [raw];
    for (const value of values) query.append(key, String(value));
  }
  return query.toString();
}

function splitUrl(url: string, params?: object): { path: string; query: string } {
  const [rawPath = '/', existing = ''] = url.split('?', 2);
  const query = new URLSearchParams(existing);
  new URLSearchParams(encodeQuery(params)).forEach((value, key) => {
    query.append(key, value);
  });
  const path = rawPath.startsWith('/') ? rawPath : `/${rawPath}`;
  return { path, query: query.toString() };
}

function parseBody(bytes: Uint8Array): unknown {
  if (bytes.byteLength === 0) return undefined;
  const text = decoder.decode(bytes);
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

export function createHttpClient(options: CreateHttpClientOptions): HttpClient {
  const {
    baseUrl,
    getAuthToken,
    getDeviceToken,
    deviceTokenHeader = DEFAULT_DEVICE_TOKEN_HEADER,
    onUnauthorized,
    timeout = DEFAULT_TIMEOUT,
    headers: defaultHeaders = {},
  } = options;
  const transport = createArena360GrpcWebTransport(baseUrl);

  function requestHeaders(overrides: Record<string, string> = {}): Record<string, string> {
    const headers = { ...defaultHeaders, ...overrides };
    const authToken = getAuthToken?.();
    if (authToken) headers.authorization = normalizeToken(authToken);
    const deviceToken = getDeviceToken?.();
    if (deviceToken) headers[deviceTokenHeader] = normalizeToken(deviceToken);
    return headers;
  }

  async function request<T>(
    method: 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE',
    url: string,
    data?: unknown,
    config: HttpRequestConfig = {},
  ): Promise<T> {
    const { path, query } = splitUrl(url, config.params);
    const headers = requestHeaders(config.headers);

    try {
      const response = await transport.invoke({
        method,
        path,
        query,
        body: encodeJson(data),
        headers,
        timeoutMs: config.timeout ?? timeout,
      });
      const body = parseBody(response.bodyJson);
      if (response.statusCode >= 400) {
        if (response.statusCode === 401) {
          onUnauthorized?.({
            url,
            message:
              body && typeof body === 'object' && 'message' in body
                ? String(body.message)
                : undefined,
            authHeader: headers.authorization,
          });
        }
        if (body && typeof body === 'object' && 'message' in body && 'statusCode' in body) {
          throw ApiError.fromErrorEnvelope(
            body as { statusCode: number; message: string; error: string; timestamp?: string },
          );
        }
        throw new ApiError({
          message: `Request failed with status ${response.statusCode}`,
          statusCode: response.statusCode,
        });
      }
      return unwrapEnvelope<T>(body);
    } catch (error) {
      if (error instanceof ApiError) throw error;
      throw new ApiError({
        message: error instanceof Error ? error.message : 'RPC request failed',
        statusCode: 503,
        cause: error,
      });
    }
  }

  return {
    get: (url, config) => request('GET', url, undefined, config),
    post: (url, data, config) => request('POST', url, data, config),
    put: (url, data, config) => request('PUT', url, data, config),
    patch: (url, data, config) => request('PATCH', url, data, config),
    delete: (url, config) => request('DELETE', url, undefined, config),
    upload: async <T = unknown>(
      url: string,
      files: File | File[],
      additionalData?: Record<string, unknown>,
    ) => {
      const formData = new FormData();
      const list = Array.isArray(files) ? files : [files];
      list.forEach((file, index) => {
        formData.append(list.length === 1 ? 'file' : `file${index}`, file);
      });
      for (const [key, value] of Object.entries(additionalData ?? {})) {
        formData.append(key, JSON.stringify(value));
      }
      // Encoding through Response preserves the browser-generated multipart
      // boundary while carrying the bytes through gRPC-Web.
      const encoded = new Response(formData);
      const contentType = encoded.headers.get('content-type') ?? 'multipart/form-data';
      const { path, query } = splitUrl(url);
      const response = await transport.invoke({
        method: 'POST',
        path,
        query,
        body: new Uint8Array(await encoded.arrayBuffer()),
        headers: requestHeaders({ 'content-type': contentType }),
        timeoutMs: timeout,
      });
      const body = parseBody(response.bodyJson);
      if (response.statusCode >= 400) {
        throw new ApiError({
          message:
            body && typeof body === 'object' && 'message' in body
              ? String(body.message)
              : `Upload failed with status ${response.statusCode}`,
          statusCode: response.statusCode,
        });
      }
      return unwrapEnvelope<T>(body);
    },
    download: async (url, filename, config) => {
      const { path, query } = splitUrl(url, config?.params);
      const response = await transport.invoke({
        method: 'GET',
        path,
        query,
        headers: requestHeaders(config?.headers),
        timeoutMs: config?.timeout ?? timeout,
      });
      if (response.statusCode >= 400) {
        throw new ApiError({ message: 'Download failed', statusCode: response.statusCode });
      }
      const objectUrl = window.URL.createObjectURL(
        new Blob([response.bodyJson], {
          type: response.headers['content-type'] ?? 'application/octet-stream',
        }),
      );
      const link = document.createElement('a');
      link.href = objectUrl;
      link.download = filename ?? 'download';
      link.click();
      window.URL.revokeObjectURL(objectUrl);
    },
  };
}
