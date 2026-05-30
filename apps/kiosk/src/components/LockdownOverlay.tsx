import type { ReactNode } from 'react';

interface LockdownOverlayProps {
  visible: boolean;
  children: ReactNode;
}

/** Full-screen shell overlay; blocks interaction with underlying OS chrome when visible */
export function LockdownOverlay({ visible, children }: LockdownOverlayProps) {
  if (!visible) return <>{children}</>;

  return (
    <div className="lockdown-root">
      <div className="lockdown-overlay" aria-hidden="true" />
      <div className="lockdown-content">{children}</div>
    </div>
  );
}
