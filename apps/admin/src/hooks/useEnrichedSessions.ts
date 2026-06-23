import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getPlayerPlans, PlayerPlanStatus } from '../services/player-plans/list';

export function useEnrichedPlayerPlans(playerId?: string) {
  const query = useQuery({
    queryKey: ['player-plans-active', playerId],
    queryFn: () =>
      getPlayerPlans({
        status: PlayerPlanStatus.ACTIVE,
        limit: 100,
        playerId: playerId as string,
      }),
    enabled: !!playerId,
  });

  const data = useMemo(() => query.data?.data ?? [], [query.data?.data]);

  return { ...query, data };
}
