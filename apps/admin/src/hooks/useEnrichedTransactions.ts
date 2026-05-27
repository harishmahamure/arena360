import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { getPlans } from '../services/plans/list';
import { getPlayers } from '../services/players/list';
import type { TransactionResponse } from '../services/transactions/list';

export function useEnrichedTransactions(transactions: TransactionResponse[] | undefined) {
  const { data: playersData } = useQuery({
    queryKey: ['players-lookup-transactions'],
    queryFn: () => getPlayers({ limit: 100 }),
    enabled: (transactions?.length ?? 0) > 0,
  });

  const { data: plansData } = useQuery({
    queryKey: ['plans-lookup-transactions'],
    queryFn: () => getPlans({ limit: 100 }),
    enabled: (transactions?.length ?? 0) > 0,
  });

  return useMemo(() => {
    if (!transactions) return [];

    const playerMap = new Map(playersData?.data?.map((p) => [p.id, p]) ?? []);
    const planMap = new Map(plansData?.data?.map((p) => [p.id, p]) ?? []);

    return transactions.map((tx) => ({
      ...tx,
      player:
        tx.player ??
        (() => {
          const p = playerMap.get(tx.playerId);
          return p
            ? { id: p.id, username: p.username, email: p.email ?? '', name: p.username }
            : undefined;
        })(),
      plan:
        tx.plan ??
        (tx.planId
          ? (() => {
              const p = planMap.get(tx.planId as string);
              return p
                ? {
                    id: p.id,
                    name: p.name,
                    planType: p.planType,
                    price: parseFloat(p.price),
                  }
                : undefined;
            })()
          : undefined),
    }));
  }, [transactions, playersData?.data, plansData?.data]);
}
