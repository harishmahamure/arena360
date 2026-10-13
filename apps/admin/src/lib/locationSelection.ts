import { panelClaims } from './authSession';

function key() {
  const claims = panelClaims();
  return `arena:location:${claims?.tenantId ?? ''}:${claims?.userId ?? ''}`;
}
export function selectedLocationId(): string {
  return sessionStorage.getItem(key()) ?? '';
}
export function rememberLocation(id: string): void {
  // Each tab keeps its operating scope when another tab signs in or chooses a venue.
  localStorage.removeItem(key());
  if (id) sessionStorage.setItem(key(), id);
  else sessionStorage.removeItem(key());
}
export function selectLocation(id: string): void {
  rememberLocation(id);
  // Recreate all query caches and forms when changing the operating location.
  window.location.reload();
}
