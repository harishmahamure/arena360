import type { AppPhase } from '../context/KioskProvider';

export type SetupShortcutAction = 'noop' | 'enterSetup' | 'focusSetup';

/** Decide how Ctrl+Shift+A should behave for the current app phase. */
export function resolveSetupShortcutAction(phase: AppPhase): SetupShortcutAction {
  if (phase === 'register' || phase === 'loading') return 'noop';
  if (phase === 'setup') return 'focusSetup';
  return 'enterSetup';
}
