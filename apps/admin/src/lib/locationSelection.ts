import { panelClaims } from './authSession';

function key() {
  const claims = panelClaims();
  return `arena:location:${claims?.tenantId ?? ''}:${claims?.userId ?? ''}`;
}
export function selectedLocationId(): string {
  return localStorage.getItem(key()) ?? '';
}
export function selectLocation(id: string): void {
  if (id) localStorage.setItem(key(), id);
  else localStorage.removeItem(key());
  // Recreate all query caches and forms when changing the operating location.
  window.location.reload();
}
