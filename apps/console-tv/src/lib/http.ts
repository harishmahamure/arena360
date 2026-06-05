import { API_BASE_URL } from './config';

interface ApiEnvelope<T> {
  success?: boolean;
  statusCode?: number;
  data: T;
}

interface ApiErrorBody {
  statusCode: number;
  message: string;
  error?: string;
  details?: unknown;
}

export class ApiError extends Error {
  readonly statusCode: number;
  readonly code?: string;
  readonly details?: unknown;

  constructor(message: string, statusCode: number, code?: string, details?: unknown) {
    super(message);
    this.name = 'ApiError';
    this.statusCode = statusCode;
    this.code = code;
    this.details = details;
  }
}

function unwrapEnvelope<T>(body: unknown): T {
  if (body && typeof body === 'object' && 'data' in body) {
    return (body as ApiEnvelope<T>).data;
  }
  return body as T;
}

async function parseError(response: Response): Promise<ApiError> {
  try {
    const body = (await response.json()) as ApiErrorBody;
    return new ApiError(
      body.message ?? response.statusText,
      body.statusCode ?? response.status,
      body.message,
      body.details,
    );
  } catch {
    return new ApiError(response.statusText || 'Request failed', response.status);
  }
}

export const tokenCache: { device?: string } = {};

export async function apiRequest<T>(
  method: string,
  path: string,
  body?: unknown,
  bearerOverride?: string,
): Promise<T> {
  const token = bearerOverride ?? tokenCache.device;
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
  };
  if (token) {
    headers.Authorization = token.startsWith('Bearer ') ? token : `Bearer ${token}`;
  }

  const response = await fetch(`${API_BASE_URL}${path}`, {
    method,
    headers,
    body: body != null ? JSON.stringify(body) : undefined,
  });

  if (!response.ok) {
    throw await parseError(response);
  }

  if (response.status === 204) {
    return undefined as T;
  }

  const json = await response.json();
  return unwrapEnvelope<T>(json);
}

export function post<T>(path: string, body: unknown, bearerOverride?: string): Promise<T> {
  return apiRequest<T>('POST', path, body, bearerOverride);
}
