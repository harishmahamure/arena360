import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getPlayerCredit } from '../services/credit';
import { getPlans } from '../services/plans/list';
import { getPlayerPlans, PlayerPlanStatus } from '../services/player-plans/list';
import { getPlayerById } from '../services/players/getById';
import { getSessions } from '../services/sessions/list';
import { useEnrichedPlayerPlans } from './useEnrichedSessions';
import { Permission, usePermissions } from './usePermissions';

export function usePlayerProfile(playerId: string | undefined) {
  const { can } = usePermissions();
  const canReadPlans = can(Permission.PlayerPlansRead);
  const canReadCredit = can(Permission.CreditRead);
  const canReadSessions = can(Permission.SessionsRead);

  const playerQuery = useQuery({
    queryKey: ['player', playerId],
    queryFn: () => getPlayerById(playerId as string),
    enabled: !!playerId,
    staleTime: 1000 * 30,
    refetchOnWindowFocus: true,
    refetchOnMount: true,
  });

  const isMember = playerQuery.data?.role === 'player';

  const activePlansQuery = useEnrichedPlayerPlans(
    isMember && canReadPlans && playerId ? playerId : undefined,
  );

  const exhaustedPlansQuery = useQuery({
    queryKey: ['player-plans-exhausted', playerId],
    queryFn: () =>
      getPlayerPlans({
        playerId: playerId as string,
        status: PlayerPlanStatus.EXHAUSTED,
        limit: 2,
        sortBy: 'updatedAt',
        sortOrder: 'DESC',
      }),
    enabled: !!playerId && isMember && canReadPlans,
  });

  const { data: plansLookup } = useQuery({
    queryKey: ['plans-lookup-exhausted'],
    queryFn: () => getPlans({ limit: 100, isActive: 1 }),
    enabled:
      !!playerId && isMember && canReadPlans && (exhaustedPlansQuery.data?.data?.length ?? 0) > 0,
  });

  const enrichedExhaustedPlans = useMemo(() => {
    if (!exhaustedPlansQuery.data?.data) return [];
    const planMap = new Map(plansLookup?.data?.map((p) => [p.id, p]) ?? []);
    return exhaustedPlansQuery.data.data.map((pp) => ({
      ...pp,
      plan: pp.plan ?? (pp.sourcePlanId ? planMap.get(pp.sourcePlanId) : undefined),
    }));
  }, [exhaustedPlansQuery.data?.data, plansLookup?.data]);

  const activeSessionQuery = useQuery({
    queryKey: ['player-active-session', playerId],
    queryFn: () =>
      getSessions({
        playerId,
        isActive: 1,
        limit: 1,
        sortBy: 'startTime',
        sortOrder: 'DESC',
      }),
    enabled: !!playerId && isMember && canReadSessions,
  });

  const recentSessionsQuery = useQuery({
    queryKey: ['player-recent-sessions', playerId],
    queryFn: () =>
      getSessions({
        playerId,
        limit: 5,
        sortBy: 'startTime',
        sortOrder: 'DESC',
      }),
    enabled: !!playerId && isMember && canReadSessions,
  });

  const creditQuery = useQuery({
    queryKey: ['player-credit-detail', playerId],
    queryFn: () => getPlayerCredit(playerId as string),
    enabled: !!playerId && isMember && canReadCredit,
  });

  const primaryActivePlan = useMemo(() => {
    const plans = activePlansQuery.data ?? [];
    if (plans.length === 0) return undefined;
    return [...plans].sort(
      (a, b) => new Date(a.expiryDate).getTime() - new Date(b.expiryDate).getTime(),
    )[0];
  }, [activePlansQuery.data]);

  const isLoading =
    playerQuery.isLoading ||
    (isMember && canReadPlans && activePlansQuery.isLoading) ||
    (isMember && canReadPlans && exhaustedPlansQuery.isLoading);

  return {
    player: playerQuery.data,
    isMember,
    canReadPlans,
    canReadCredit,
    canReadSessions,
    activePlans: activePlansQuery.data ?? [],
    exhaustedPlans: enrichedExhaustedPlans,
    primaryActivePlan,
    activeSession: activeSessionQuery.data?.data?.[0],
    recentSessions: recentSessionsQuery.data?.data ?? [],
    credit: creditQuery.data,
    isLoading,
    error: playerQuery.error,
    refetchPlayer: playerQuery.refetch,
  };
}
