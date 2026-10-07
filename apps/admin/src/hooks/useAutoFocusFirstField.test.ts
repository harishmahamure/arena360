import { afterEach, describe, expect, it, vi } from 'vitest';
import { findFirstField, isFormRoute } from './useAutoFocusFirstField';

function render(html: string) {
  document.body.innerHTML = html;
  vi.spyOn(HTMLElement.prototype, 'getClientRects').mockReturnValue([{}] as unknown as DOMRectList);
  return document.body;
}

describe('auto focus', () => {
  afterEach(() => {
    vi.restoreAllMocks();
    document.body.innerHTML = '';
  });

  it('treats create, edit, setup and login screens as forms', () => {
    expect(isFormRoute('/players/new')).toBe(true);
    expect(isFormRoute('/products/42/edit')).toBe(true);
    expect(isFormRoute('/shift/setup')).toBe(true);
    expect(isFormRoute('/login')).toBe(true);
    expect(isFormRoute('/players')).toBe(false);
  });

  it('skips hidden, disabled and read-only inputs', () => {
    const root = render(`
      <input type="hidden" />
      <input disabled />
      <input readonly />
      <input type="checkbox" />
      <input id="name" />
      <textarea></textarea>`);
    expect(findFirstField(root)?.id).toBe('name');
  });

  it('prefers an explicit data-autofocus target', () => {
    const root = render('<input id="player" /><input id="search" data-autofocus />');
    expect(findFirstField(root)?.id).toBe('search');
  });
});
