import { http, local } from '@gaming-cafe/utils';
import { useQuery } from '@tanstack/react-query';
import { useEffect } from 'react';
import { useDispatch, useSelector } from '../hooks/store';
import type { VerifyOtpResponseUser } from '../services/auth/types';

/** Revalidate active accounts on focus and periodically, even when the page is idle. */
export default function SessionVerifier() {
  const auth = useSelector((state) => state.auth);
  const dispatch = useDispatch();
  const token = local.get<string>('accessToken');
  const { data } = useQuery({
    queryKey: ['current-session', auth.id],
    queryFn: () => http.get<VerifyOtpResponseUser>('/auth/me'),
    enabled: Boolean(auth.id && token),
    staleTime: 0,
    refetchInterval: 60_000,
    refetchOnWindowFocus: 'always',
    retry: false,
  });
  useEffect(() => {
    if (!data || local.get('accessToken') !== token || data.id !== auth.id) return;
    const profile = {
      id: data.id,
      username: data.username,
      email: data.email ?? '',
      firstName: data.firstName ?? '',
      lastName: data.lastName ?? '',
      role: data.role,
      isActive: data.isActive,
    };
    if (Object.entries(profile).some(([key, value]) => auth[key as keyof typeof auth] !== value))
      dispatch({ type: 'SetAuthDetail', payload: profile });
  }, [data, token, auth, dispatch]);
  return null;
}
