import { act, fireEvent, render } from '@testing-library/react';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { DecorativeVideo } from './DecorativeVideo';

const native = vi.hoisted(() => ({
  enabled: false,
  onFocusChanged: vi.fn(),
  isFocused: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({ isTauri: () => native.enabled }));
vi.mock('@tauri-apps/api/window', () => ({ getCurrentWindow: () => native }));

function getVideo(container: HTMLElement): HTMLVideoElement {
  const video = container.querySelector('video');
  if (!video) throw new Error('Expected a decorative video');
  return video;
}

describe('DecorativeVideo', () => {
  beforeEach(() => {
    native.enabled = false;
    vi.clearAllMocks();
    vi.spyOn(document, 'hasFocus').mockReturnValue(true);
    vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('visible');
  });

  it('pauses on blur, stays paused after loaded-data, and resumes on focus', () => {
    const { container } = render(<DecorativeVideo src="/preview.webm" />);
    const video = getVideo(container);
    expect(video.play).toHaveBeenCalled();
    vi.mocked(video.play).mockClear();

    fireEvent.blur(window);
    expect(video.pause).toHaveBeenCalled();
    fireEvent.loadedData(video);
    expect(video.play).not.toHaveBeenCalled();

    fireEvent.focus(window);
    expect(video.play).toHaveBeenCalledTimes(1);
  });

  it('does not start decoding while hidden and pauses when hidden again', () => {
    const visibility = vi.spyOn(document, 'visibilityState', 'get').mockReturnValue('hidden');
    const { container } = render(<DecorativeVideo src="/preview.webm" />);
    const video = getVideo(container);
    expect(video).not.toHaveAttribute('autoplay');
    expect(video).toHaveAttribute('preload', 'none');
    expect(video.play).not.toHaveBeenCalled();

    visibility.mockReturnValue('visible');
    fireEvent(document, new Event('visibilitychange'));
    expect(video.play).toHaveBeenCalledTimes(1);
    vi.mocked(video.pause).mockClear();
    visibility.mockReturnValue('hidden');
    fireEvent(document, new Event('visibilitychange'));
    expect(video.pause).toHaveBeenCalled();
  });

  it('uses native focus and ignores a stale initial focus response', async () => {
    native.enabled = true;
    let reportFocus: (event: { payload: boolean }) => void = () => {};
    const stop = vi.fn();
    native.onFocusChanged.mockImplementation((handler) => {
      reportFocus = handler;
      return Promise.resolve(stop);
    });
    let resolveFocus: (focused: boolean) => void = () => {};
    native.isFocused.mockReturnValue(
      new Promise<boolean>((resolve) => {
        resolveFocus = resolve;
      }),
    );
    const { container, unmount } = render(<DecorativeVideo src="/preview.webm" />);
    const video = getVideo(container);
    await act(async () => reportFocus({ payload: false }));
    vi.mocked(video.play).mockClear();
    await act(async () => resolveFocus(true));
    fireEvent.loadedData(video);
    expect(video.play).not.toHaveBeenCalled();
    await act(async () => reportFocus({ payload: true }));
    expect(video.play).toHaveBeenCalledTimes(1);
    unmount();
    expect(stop).toHaveBeenCalledTimes(1);
  });

  it('cleans up a native listener even if registration finishes after unmount', async () => {
    native.enabled = true;
    const stop = vi.fn();
    let registered: (stop: () => void) => void = () => {};
    native.onFocusChanged.mockReturnValue(
      new Promise<() => void>((resolve) => {
        registered = resolve;
      }),
    );
    native.isFocused.mockResolvedValue(true);
    const { unmount } = render(<DecorativeVideo src="/preview.webm" />);
    unmount();
    await act(async () => registered(stop));
    expect(stop).toHaveBeenCalledTimes(1);
  });
});
