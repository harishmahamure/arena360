import { type RefObject, useEffect } from 'react';

const FIELD_SELECTOR = [
  'input:not([type="hidden"]):not([type="checkbox"]):not([type="radio"]):not([type="file"]):not([type="color"])',
  'textarea',
  '[role="combobox"]',
].join(',');

/** Routes that are data-entry forms: create, edit and setup screens. */
export function isFormRoute(pathname: string): boolean {
  return /\/(new|edit|setup)$/.test(pathname) || pathname === '/login';
}

function usable(element: HTMLElement): boolean {
  if (element.closest('[aria-hidden="true"], fieldset:disabled')) return false;
  if ((element as HTMLInputElement).disabled || (element as HTMLInputElement).readOnly)
    return false;
  if (element.getAttribute('aria-disabled') === 'true') return false;
  return element.getClientRects().length > 0;
}

/** An explicit `data-autofocus` target wins; otherwise the first usable field in document order. */
export function findFirstField(root: ParentNode): HTMLElement | null {
  const preferred = root.querySelector<HTMLElement>('[data-autofocus]');
  if (preferred && usable(preferred)) return preferred;
  for (const element of root.querySelectorAll<HTMLElement>(FIELD_SELECTOR)) {
    if (usable(element)) return element;
  }
  return null;
}

function userIsTyping(): boolean {
  const active = document.activeElement;
  return (
    !!active && active !== document.body && active.matches(`${FIELD_SELECTOR}, [contenteditable]`)
  );
}

/**
 * Focus the first field of a form screen once it has rendered. Pages load lazily and
 * fetch data first, so wait (briefly) for the field to appear; never steal focus from
 * something the user has already started using.
 */
export function useAutoFocusFirstField(
  container: RefObject<HTMLElement | null>,
  routeKey: string,
  enabled: boolean,
) {
  // biome-ignore lint/correctness/useExhaustiveDependencies: Re-run per route; the ref is stable.
  useEffect(() => {
    const root = container.current;
    if (!enabled || !root) return;
    let done = false;
    const stop = () => {
      done = true;
      observer.disconnect();
      clearTimeout(timeout);
      root.removeEventListener('pointerdown', stop, true);
      root.removeEventListener('keydown', stop, true);
    };
    const attempt = () => {
      if (done) return;
      if (userIsTyping()) return stop();
      const field = findFirstField(root);
      if (!field) return;
      field.focus({ preventScroll: true });
      stop();
    };
    const observer = new MutationObserver(attempt);
    observer.observe(root, { childList: true, subtree: true, attributes: true });
    const timeout = setTimeout(stop, 3000);
    root.addEventListener('pointerdown', stop, true);
    root.addEventListener('keydown', stop, true);
    attempt();
    return stop;
  }, [routeKey, enabled]);
}
