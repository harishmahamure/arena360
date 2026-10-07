/** Carry applied reporting filters between business subpages, never into other modules. */
export function analyticsNavigationPath(
  destination: string,
  location: { pathname: string; search: string },
): string {
  const isAnalytics = (path: string) => path === '/analytics' || path.startsWith('/analytics/');
  if (!isAnalytics(location.pathname) || !isAnalytics(destination)) return destination;
  const current = new URLSearchParams(location.search);
  const retained = new URLSearchParams();
  for (const key of ['startDate', 'endDate', 'compare']) {
    const value = current.get(key);
    if (value !== null) retained.set(key, value);
  }
  return `${destination}${retained.size ? `?${retained}` : ''}`;
}
