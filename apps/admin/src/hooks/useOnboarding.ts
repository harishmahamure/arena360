import { Permission } from '@gaming-cafe/contracts';
import { useQuery } from '@tanstack/react-query';
import { panelClaims } from '../lib/authSession';
import { getOnboarding } from '../services/onboarding';
import { usePermissions } from './usePermissions';

export function useOnboarding() {
  const { can, role } = usePermissions();
  const claims = panelClaims();
  const canRead = role === 'admin' && can(Permission.SettingsRead);
  const canWrite = canRead && can(Permission.SettingsWrite);
  const queryKey = ['onboarding', claims?.tenantId, claims?.userId];
  const query = useQuery({
    queryKey,
    queryFn: getOnboarding,
    enabled: canRead,
    retry: false,
  });
  return { ...query, queryKey, canRead, canWrite };
}
