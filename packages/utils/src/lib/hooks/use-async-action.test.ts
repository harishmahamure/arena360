import { act, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useAsyncAction } from './use-async-action';

describe('useAsyncAction', () => {
  it('runs fn once and returns its result', async () => {
    const { result } = renderHook(() => useAsyncAction());
    const fn = vi.fn().mockResolvedValue('ok');

    let value: string | undefined;
    await act(async () => {
      value = await result.current.run(fn);
    });

    expect(fn).toHaveBeenCalledTimes(1);
    expect(value).toBe('ok');
    expect(result.current.loading).toBe(false);
    expect(result.current.succeeded).toBe(false);
  });

  it('ignores concurrent run calls while in flight', async () => {
    const { result } = renderHook(() => useAsyncAction());
    let resolveFirst: (() => void) | undefined;
    const fn = vi.fn(
      () =>
        new Promise<string>((resolve) => {
          resolveFirst = () => resolve('done');
        }),
    );

    let firstPromise: Promise<string | undefined> | undefined;
    let secondResult: string | undefined;

    act(() => {
      firstPromise = result.current.run(fn);
    });
    expect(result.current.loading).toBe(true);

    await act(async () => {
      secondResult = await result.current.run(fn);
    });

    expect(fn).toHaveBeenCalledTimes(1);
    expect(secondResult).toBeUndefined();

    await act(async () => {
      resolveFirst?.();
      await firstPromise;
    });

    expect(result.current.loading).toBe(false);
  });

  it('clears loading after error without setting succeeded', async () => {
    const { result } = renderHook(() => useAsyncAction({ throttleMs: 0, lockOnSuccess: true }));
    const fn = vi.fn().mockRejectedValue(new Error('fail'));

    await act(async () => {
      await expect(result.current.run(fn)).rejects.toThrow('fail');
    });

    expect(fn).toHaveBeenCalledTimes(1);
    expect(result.current.loading).toBe(false);
    expect(result.current.succeeded).toBe(false);
    expect(result.current.failed).toBe(true);
    expect(result.current.errorMessage).toBe('fail');
    expect(result.current.disabled).toBe(false);
  });

  it('clears error state on next accepted run', async () => {
    const { result } = renderHook(() => useAsyncAction());
    const failFn = vi.fn().mockRejectedValue(new Error('fail'));
    const okFn = vi.fn().mockResolvedValue('ok');

    await act(async () => {
      await expect(result.current.run(failFn)).rejects.toThrow('fail');
    });
    expect(result.current.failed).toBe(true);

    await act(async () => {
      await result.current.run(okFn);
    });

    expect(result.current.failed).toBe(false);
    expect(result.current.errorMessage).toBeNull();
  });

  it('clearError clears failed state', async () => {
    const { result } = renderHook(() => useAsyncAction());
    const fn = vi.fn().mockRejectedValue(new Error('fail'));

    await act(async () => {
      await expect(result.current.run(fn)).rejects.toThrow('fail');
    });

    act(() => {
      result.current.clearError();
    });

    expect(result.current.failed).toBe(false);
    expect(result.current.errorMessage).toBeNull();
  });

  it('reset clears error state', async () => {
    const { result } = renderHook(() => useAsyncAction());
    const fn = vi.fn().mockRejectedValue(new Error('fail'));

    await act(async () => {
      await expect(result.current.run(fn)).rejects.toThrow('fail');
    });

    act(() => {
      result.current.reset();
    });

    expect(result.current.failed).toBe(false);
    expect(result.current.errorMessage).toBeNull();
  });

  it('locks on success when lockOnSuccess is true', async () => {
    const { result } = renderHook(() => useAsyncAction({ throttleMs: 0, lockOnSuccess: true }));
    const fn = vi.fn().mockResolvedValue('ok');

    await act(async () => {
      await result.current.run(fn);
    });

    expect(result.current.succeeded).toBe(true);
    expect(result.current.disabled).toBe(true);

    await act(async () => {
      await result.current.run(fn);
    });

    expect(fn).toHaveBeenCalledTimes(1);
  });

  it('reset clears succeeded state', async () => {
    const { result } = renderHook(() => useAsyncAction({ throttleMs: 0, lockOnSuccess: true }));
    const fn = vi.fn().mockResolvedValue('ok');

    await act(async () => {
      await result.current.run(fn);
    });
    expect(result.current.succeeded).toBe(true);

    act(() => {
      result.current.reset();
    });

    expect(result.current.succeeded).toBe(false);
    expect(result.current.disabled).toBe(false);
  });

  describe('throttle', () => {
    beforeEach(() => {
      vi.useFakeTimers();
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it('ignores second run within throttleMs after first completes', async () => {
      const { result } = renderHook(() => useAsyncAction({ throttleMs: 1000 }));
      const fn = vi.fn().mockResolvedValue('ok');

      await act(async () => {
        await result.current.run(fn);
      });
      expect(fn).toHaveBeenCalledTimes(1);

      await act(async () => {
        await result.current.run(fn);
      });
      expect(fn).toHaveBeenCalledTimes(1);

      await act(async () => {
        vi.advanceTimersByTime(1000);
        await result.current.run(fn);
      });
      expect(fn).toHaveBeenCalledTimes(2);
    });
  });

  it('runIf skips fn when predicate is false', async () => {
    const { result } = renderHook(() => useAsyncAction());
    const fn = vi.fn().mockResolvedValue('ok');

    let value: string | undefined;
    await act(async () => {
      value = await result.current.runIf(() => false, fn);
    });

    expect(fn).not.toHaveBeenCalled();
    expect(value).toBeUndefined();
  });

  it('runIf runs fn when predicate is true', async () => {
    const { result } = renderHook(() => useAsyncAction());
    const fn = vi.fn().mockResolvedValue('ok');

    let value: string | undefined;
    await act(async () => {
      value = await result.current.runIf(() => true, fn);
    });

    expect(fn).toHaveBeenCalledTimes(1);
    expect(value).toBe('ok');
  });
});
