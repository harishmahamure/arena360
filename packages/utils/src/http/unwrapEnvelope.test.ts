import { describe, expect, it } from 'vitest';
import { ApiError, isApiError } from './ApiError';
import { unwrapEnvelope } from './unwrapEnvelope';

describe('unwrapEnvelope', () => {
  it('returns inner data from a success envelope', () => {
    const envelope = {
      success: true,
      statusCode: 200,
      timestamp: '2026-05-27T08:00:00Z',
      data: { id: '1', name: 'Test Plan' },
    };

    expect(unwrapEnvelope(envelope)).toEqual({ id: '1', name: 'Test Plan' });
  });

  it('returns pagination page data from a success envelope', () => {
    const envelope = {
      success: true,
      statusCode: 200,
      timestamp: '2026-05-27T08:00:00Z',
      data: {
        data: [{ id: 'plan-1' }],
        total: 1,
        page: 1,
        limit: 20,
        totalPages: 1,
      },
    };

    expect(unwrapEnvelope(envelope)).toEqual({
      data: [{ id: 'plan-1' }],
      total: 1,
      page: 1,
      limit: 20,
      totalPages: 1,
    });
  });

  it('throws ApiError for an error envelope', () => {
    const envelope = {
      statusCode: 404,
      message: 'Plan not found',
      error: 'Not Found',
      timestamp: '2026-05-27T08:00:00Z',
    };

    expect(() => unwrapEnvelope(envelope)).toThrow(ApiError);

    try {
      unwrapEnvelope(envelope);
    } catch (error) {
      expect(isApiError(error)).toBe(true);
      if (isApiError(error)) {
        expect(error.statusCode).toBe(404);
        expect(error.message).toBe('Plan not found');
        expect(error.errorLabel).toBe('Not Found');
        expect(error.timestamp).toBe('2026-05-27T08:00:00Z');
      }
    }
  });

  it('throws ApiError for unexpected response shapes', () => {
    expect(() => unwrapEnvelope({ foo: 'bar' })).toThrow(ApiError);
  });
});
