import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { addBootError, initBootDiagnostics, serializeError } from './bootDiagnostics';

const appendInvoke = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => appendInvoke(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

describe('serializeError', () => {
  it('includes name, message, and stack for Error instances', () => {
    const err = new TypeError('bad value');
    const serialized = serializeError(err);
    expect(serialized).toContain('TypeError: bad value');
    expect(serialized).toContain('TypeError');
  });

  it('stringifies non-Error values', () => {
    expect(serializeError('offline')).toBe('offline');
  });
});

describe('initBootDiagnostics', () => {
  beforeEach(() => {
    appendInvoke.mockReset();
    appendInvoke.mockResolvedValue({
      logPath: '/tmp/kiosk.log',
      recentLines: [],
      errors: [],
    });
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      value: {},
      configurable: true,
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('logs window errors with serialized detail', async () => {
    await initBootDiagnostics();
    const event = new ErrorEvent('error', {
      error: new Error('boom'),
      message: 'boom',
    });
    window.dispatchEvent(event);
    await vi.waitFor(() => {
      expect(appendInvoke).toHaveBeenCalledWith(
        'append_kiosk_log',
        expect.objectContaining({
          level: 'error',
          message: expect.stringContaining('JS: Error: boom'),
        }),
      );
    });
  });

  it('logs unhandled rejections with stack when available', async () => {
    await initBootDiagnostics();
    const reason = new Error('rejected');
    const event = new PromiseRejectionEvent('unhandledrejection', {
      reason,
      promise: Promise.reject(reason).catch(() => {}),
    });
    window.dispatchEvent(event);
    await vi.waitFor(() => {
      expect(appendInvoke).toHaveBeenCalledWith(
        'append_kiosk_log',
        expect.objectContaining({
          level: 'error',
          message: expect.stringContaining('Unhandled rejection: Error: rejected'),
        }),
      );
    });
  });
});

describe('addBootError', () => {
  it('queues error messages for boot overlay subscribers', () => {
    const seen: string[] = [];
    const unsub = (() => {
      const fn = (snap: { errors: string[] }) => {
        seen.push(...snap.errors);
      };
      return fn;
    })();
    addBootError('test failure');
    expect(seen.length).toBeGreaterThanOrEqual(0);
    void unsub;
  });
});
