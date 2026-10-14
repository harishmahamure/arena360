import { isTauri } from '@tauri-apps/api/core';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useCallback, useEffect, useRef, useState, type VideoHTMLAttributes } from 'react';

type DecorativeVideoProps = Pick<
  VideoHTMLAttributes<HTMLVideoElement>,
  'src' | 'className' | 'poster' | 'onError'
>;

/** Pause decorative media whenever another application has the player's attention. */
export function DecorativeVideo(props: DecorativeVideoProps) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const [foreground, setForeground] = useState(
    () => document.visibilityState === 'visible' && document.hasFocus(),
  );
  const foregroundRef = useRef(foreground);
  foregroundRef.current = foreground;

  useEffect(() => {
    let disposed = false;
    let focused = document.hasFocus();
    let focusRevision = 0;
    let unlisten: (() => void) | undefined;
    const sync = () => {
      if (disposed) return;
      foregroundRef.current = document.visibilityState === 'visible' && focused;
      setForeground(foregroundRef.current);
    };
    const onFocus = () => {
      focusRevision++;
      focused = true;
      sync();
    };
    const onBlur = () => {
      focusRevision++;
      focused = false;
      sync();
    };
    window.addEventListener('focus', onFocus);
    window.addEventListener('blur', onBlur);
    document.addEventListener('visibilitychange', sync);
    sync();

    // Native focus also covers WebView2 while a fullscreen game is in front.
    if (isTauri()) {
      const nativeWindow = getCurrentWindow();
      void nativeWindow
        .onFocusChanged(({ payload }) => {
          if (payload) onFocus();
          else onBlur();
        })
        .then((stop) => {
          if (disposed) stop();
          else unlisten = stop;
        })
        .catch(() => {});
      const revision = focusRevision;
      void nativeWindow
        .isFocused()
        .then((value) => {
          if (disposed || revision !== focusRevision) return;
          focused = value;
          sync();
        })
        .catch(() => {});
    }

    return () => {
      disposed = true;
      foregroundRef.current = false;
      unlisten?.();
      window.removeEventListener('focus', onFocus);
      window.removeEventListener('blur', onBlur);
      document.removeEventListener('visibilitychange', sync);
    };
  }, []);

  const syncPlayback = useCallback(() => {
    const video = videoRef.current;
    if (!video || !props.src) return;
    if (!foreground || !foregroundRef.current) {
      video.pause();
      return;
    }
    void video
      .play()
      .then(() => {
        // Focus can change while the browser is starting playback.
        if (!foregroundRef.current) video.pause();
      })
      .catch(() => {});
  }, [foreground, props.src]);

  useEffect(() => {
    syncPlayback();
    const video = videoRef.current;
    return () => video?.pause();
  }, [syncPlayback]);

  return (
    <video
      {...props}
      ref={videoRef}
      loop
      muted
      playsInline
      preload="none"
      onLoadedData={syncPlayback}
    />
  );
}
