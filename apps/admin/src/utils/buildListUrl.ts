/**
 * Build a list page URL preserving filter query params (e.g. active, role, status).
 */
export function buildListUrl(
  basePath: string,
  page: number,
  filters: Record<string, string | undefined>,
): string {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(filters)) {
    if (value != null && value !== '') {
      params.set(key, value);
    }
  }
  if (page > 1) {
    params.set('page', String(page));
  }
  const query = params.toString();
  return query ? `${basePath}?${query}` : basePath;
}
