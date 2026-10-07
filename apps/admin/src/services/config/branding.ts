import { http } from '@gaming-cafe/utils';
import { useQuery } from '@tanstack/react-query';

export interface Branding {
  name: string;
  logoUrl: string;
  primaryColor: string;
}

export const DEFAULT_BRAND_NAME = 'Arena360';

export const getBranding = () => http.get<Branding>('/branding');

/** Public white-label identity; safe to call before sign-in. */
export function useBranding(): Branding {
  const { data } = useQuery({
    queryKey: ['branding'],
    queryFn: getBranding,
    staleTime: 5 * 60_000,
    retry: false,
  });
  return {
    name: data?.name.trim() || DEFAULT_BRAND_NAME,
    logoUrl: data?.logoUrl ?? '',
    primaryColor: /^#[0-9a-f]{6}$/i.test(data?.primaryColor ?? '')
      ? (data?.primaryColor ?? '')
      : '',
  };
}
