import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getDevices } from '../services/devices/list';
import { getPlans } from '../services/plans/list';
import { getPlayerPlans, PlayerPlanStatus } from '../services/player-plans/list';
import { getPlayers } from '../services/players/list';
import type { SessionResponse } from '../services/sessions/list';

export function useEnrichedPlayerPlans(playerId?: string) {
  const { data: playerPlansData, ...rest } = useQuery({
    queryKey: ['player-plans-active', playerId],
    queryFn: () =>
      getPlayerPlans({
        status: PlayerPlanStatus.ACTIVE,
        limit: 100,
        playerId: playerId as string,
      }),
    enabled: !!playerId,
  });

  const { data: playersData } = useQuery({
    queryKey: ['players-lookup'],
    queryFn: () => getPlayers({ limit: 100 }),
    enabled: !!playerId && (playerPlansData?.data?.length ?? 0) > 0,
  });

  const { data: plansData } = useQuery({
    queryKey: ['plans-lookup'],
    queryFn: () => getPlans({ limit: 100, isActive: 1 }),
    enabled: !!playerId && (playerPlansData?.data?.length ?? 0) > 0,
  });

  const enrichedPlans = useMemo(() => {
    if (!playerPlansData?.data) return [];

    const playerMap = new Map(playersData?.data?.map((p) => [p.id, p]) ?? []);
    const planMap = new Map(plansData?.data?.map((p) => [p.id, p]) ?? []);

    return playerPlansData.data.map((pp) => ({
      ...pp,
      player: pp.player ?? playerMap.get(pp.playerId),
      plan: pp.plan ?? planMap.get(pp.planId),
    }));
  }, [playerPlansData?.data, playersData?.data, plansData?.data]);

  return { data: enrichedPlans, ...rest };
}

export function useEnrichedSessions(sessions: SessionResponse[] | undefined) {
  const { data: devicesData } = useQuery({
    queryKey: ['devices-lookup'],
    queryFn: () => getDevices({ limit: 100 }),
    enabled: (sessions?.length ?? 0) > 0,
  });

  const { data: playerPlansData } = useQuery({
    queryKey: ['player-plans-lookup'],
    queryFn: () => getPlayerPlans({ limit: 100 }),
    enabled: (sessions?.length ?? 0) > 0,
  });

  const { data: playersData } = useQuery({
    queryKey: ['players-lookup-sessions'],
    queryFn: () => getPlayers({ limit: 100 }),
    enabled: (sessions?.length ?? 0) > 0,
  });

  const { data: plansData } = useQuery({
    queryKey: ['plans-lookup-sessions'],
    queryFn: () => getPlans({ limit: 100 }),
    enabled: (sessions?.length ?? 0) > 0,
  });

  return useMemo(() => {
    if (!sessions) return [];

    const deviceMap = new Map(devicesData?.data?.map((d) => [d.id, d]) ?? []);
    const playerPlanMap = new Map(playerPlansData?.data?.map((pp) => [pp.id, pp]) ?? []);
    const playerMap = new Map(playersData?.data?.map((p) => [p.id, p]) ?? []);
    const planMap = new Map(plansData?.data?.map((p) => [p.id, p]) ?? []);

    return sessions.map((session): SessionResponse => {
      const flatPlan = playerPlanMap.get(session.playerPlanId);
      const player = flatPlan?.player ?? (flatPlan ? playerMap.get(flatPlan.playerId) : undefined);
      const plan = flatPlan?.plan ?? (flatPlan ? planMap.get(flatPlan.planId) : undefined);

      const enrichedPlayerPlan =
        session.playerPlan ??
        (flatPlan
          ? {
              id: flatPlan.id,
              playerId: flatPlan.playerId,
              planId: flatPlan.planId,
              status: flatPlan.status,
              remainingTimeCredits: flatPlan.remainingTimeCredits,
              player: player
                ? {
                    id: player.id,
                    username: player.username,
                    firstName: player.firstName,
                    lastName: player.lastName,
                  }
                : flatPlan.player,
              plan: plan
                ? {
                    id: plan.id,
                    name: plan.name,
                    planType: plan.planType,
                    timeCredits: plan.timeCredits ?? 0,
                  }
                : flatPlan.plan
                  ? {
                      id: flatPlan.plan.id,
                      name: flatPlan.plan.name,
                      planType: flatPlan.plan.planType,
                      timeCredits: flatPlan.plan.timeCredits,
                    }
                  : undefined,
            }
          : undefined);

      return {
        ...session,
        device: session.device ?? deviceMap.get(session.deviceId),
        playerPlan: enrichedPlayerPlan,
      };
    });
  }, [sessions, devicesData?.data, playerPlansData?.data, playersData?.data, plansData?.data]);
}
