import { useEffect } from 'react';

/** Block OS/browser context menus while the kiosk shell is locked (kiosk-only actions). */
export function useKioskShellGuard(enabled: boolean) {
  useEffect(() => {
    if (!enabled) return;
    function onContextMenu(e: MouseEvent) {
      e.preventDefault();
    }
    window.addEventListener('contextmenu', onContextMenu);
    return () => window.removeEventListener('contextmenu', onContextMenu);
  }, [enabled]);
}
