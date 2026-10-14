import { useEffect } from 'react';
import type { AppPhase } from '../context/KioskProvider';
import { allowListPaths } from '../lib/allowList';
import { appendKioskLog } from '../lib/bootDiagnostics';
import { setSignedOutCleanupEnabled } from '../lib/tauriCommands';

export function signedOutCleanupEnabled(
  phase: AppPhase,
  hasSession: boolean,
  signingIn: boolean,
): boolean {
  return (
    !hasSession &&
    !signingIn &&
    [
      'login',
      'register',
      'create-account',
      'create-account-success',
      'already-in-session',
    ].includes(phase)
  );
}

export function useSignedOutCleanup(
  phase: AppPhase,
  hasSession: boolean,
  signingIn: boolean,
): void {
  const enabled = signedOutCleanupEnabled(phase, hasSession, signingIn);
  useEffect(() => {
    void setSignedOutCleanupEnabled(enabled, enabled ? allowListPaths() : []).catch((e) => {
      void appendKioskLog('warn', `[idle cleanup] state sync failed: ${String(e)}`);
    });
    return () => {
      void setSignedOutCleanupEnabled(false).catch(() => {});
    };
  }, [enabled]);
}
