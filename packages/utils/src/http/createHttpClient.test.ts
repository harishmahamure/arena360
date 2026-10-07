import { beforeEach, describe, expect, it, vi } from 'vitest';
import { createHttpClient } from './createHttpClient';

const invoke = vi.hoisted(() => vi.fn());
vi.mock('@gaming-cafe/proto', () => ({
  createArena360GrpcWebTransport: () => ({ invoke }),
  encodeJson: (value: unknown) => new TextEncoder().encode(JSON.stringify(value)),
}));
const response = (statusCode: number, data: unknown) => ({
  statusCode,
  headers: {},
  bodyJson: new TextEncoder().encode(JSON.stringify(data)),
});

describe('authenticated RPC operations', () => {
  beforeEach(() => vi.resetAllMocks());
  it('unwraps successful data and preserves request identity', async () => {
    invoke.mockResolvedValue(response(200, { success: true, data: { id: 'saved' } }));
    const http = createHttpClient({ baseUrl: 'http://localhost', getAuthToken: () => 'token' });
    expect(await http.put('/settings?scope=one', { value: 2 })).toEqual({ id: 'saved' });
    expect(invoke).toHaveBeenCalledWith(
      expect.objectContaining({
        method: 'PUT',
        path: '/settings',
        query: 'scope=one',
        headers: { authorization: 'Bearer token' },
      }),
    );
  });
  it('passes the original request token to 401 handling after a token changes', async () => {
    let token = 'old';
    let resolve!: (value: unknown) => void;
    invoke.mockImplementation(
      () =>
        new Promise((r) => {
          resolve = r;
        }),
    );
    const onUnauthorized = vi.fn();
    const http = createHttpClient({
      baseUrl: 'http://localhost',
      getAuthToken: () => token,
      onUnauthorized,
    });
    const pending = http.get('/users');
    token = 'new';
    resolve(response(401, { message: 'Account disabled' }));
    await expect(pending).rejects.toMatchObject({ statusCode: 401 });
    expect(onUnauthorized).toHaveBeenCalledWith(
      expect.objectContaining({ authHeader: 'Bearer old' }),
    );
  });
  it('does not log out on permission denials', async () => {
    invoke.mockResolvedValue(response(403, { message: 'Forbidden' }));
    const onUnauthorized = vi.fn();
    await expect(
      createHttpClient({ baseUrl: 'http://localhost', onUnauthorized }).get('/config'),
    ).rejects.toMatchObject({ statusCode: 403 });
    expect(onUnauthorized).not.toHaveBeenCalled();
  });
  it('handles gRPC unauthenticated failures and preserves permission-denied status', async () => {
    const onUnauthorized = vi.fn();
    const http = createHttpClient({ baseUrl: 'http://localhost', onUnauthorized });
    invoke.mockRejectedValue(Object.assign(new Error('Unauthenticated'), { code: 16 }));
    await expect(http.get('/users')).rejects.toMatchObject({ statusCode: 401 });
    expect(onUnauthorized).toHaveBeenCalledOnce();
    invoke.mockRejectedValue(Object.assign(new Error('Permission denied'), { code: 7 }));
    await expect(http.get('/users')).rejects.toMatchObject({ statusCode: 403 });
    expect(onUnauthorized).toHaveBeenCalledOnce();
  });
  it('applies the same unauthorized handling to uploads and downloads', async () => {
    invoke.mockResolvedValue(response(401, { message: 'Expired' }));
    const onUnauthorized = vi.fn();
    const http = createHttpClient({
      baseUrl: 'http://localhost',
      onUnauthorized,
      getAuthToken: () => 'token',
    });
    await expect(http.upload('/uploads', new File(['contents'], 'test.txt'))).rejects.toMatchObject(
      { statusCode: 401 },
    );
    await expect(http.download('/exports')).rejects.toMatchObject({ statusCode: 401 });
    expect(onUnauthorized).toHaveBeenCalledTimes(2);
  });
  it('refreshes consumers after successful mutations, never failed writes or reads', async () => {
    const onMutationSuccess = vi.fn();
    const http = createHttpClient({ baseUrl: 'http://localhost', onMutationSuccess });
    invoke.mockResolvedValue(response(200, { success: true, data: { id: 'saved' } }));
    await http.post('/players', {});
    expect(onMutationSuccess).toHaveBeenCalledWith({ url: '/players', method: 'POST' });
    await http.get('/players');
    invoke.mockResolvedValue(response(409, { message: 'Conflict' }));
    await expect(http.put('/players', {})).rejects.toMatchObject({ statusCode: 409 });
    expect(onMutationSuccess).toHaveBeenCalledOnce();
  });
});
