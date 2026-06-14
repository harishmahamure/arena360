import { describe, expect, it } from 'vitest';
import { resolveSetupShortcutAction } from './setupShortcut';

describe('resolveSetupShortcutAction', () => {
  it('returns noop for register and loading', () => {
    expect(resolveSetupShortcutAction('register')).toBe('noop');
    expect(resolveSetupShortcutAction('loading')).toBe('noop');
  });

  it('returns focusSetup when setup is already open', () => {
    expect(resolveSetupShortcutAction('setup')).toBe('focusSetup');
  });

  it('returns enterSetup for login, session, and already-in-session', () => {
    expect(resolveSetupShortcutAction('login')).toBe('enterSetup');
    expect(resolveSetupShortcutAction('session')).toBe('enterSetup');
    expect(resolveSetupShortcutAction('already-in-session')).toBe('enterSetup');
  });
});
