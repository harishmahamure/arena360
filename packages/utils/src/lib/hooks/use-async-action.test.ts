import { act, renderHook } from '@testing-library/react';
import { describe, expect, it, vi } from 'vitest';
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

  it('clears loading after error', async () => {
    const { result } = renderHook(() => useAsyncAction());
    const fn = vi.fn().mockRejectedValue(new Error('fail'));

    await act(async () => {
      await expect(result.current.run(fn)).rejects.toThrow('fail');
    });

    expect(fn).toHaveBeenCalledTimes(1);
    expect(result.current.loading).toBe(false);
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
    expect(result.current.loading).toBe(false);
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
