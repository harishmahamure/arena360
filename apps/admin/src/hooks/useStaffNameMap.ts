import { useQuery } from '@tanstack/react-query';
import { getPlayers } from '../services/players/list';

function displayName(firstName?: string | null, lastName?: string | null, username?: string) {
  const full = [firstName, lastName].filter(Boolean).join(' ').trim();
  return full || username || 'Unknown';
}

export function useStaffNameMap() {
  const { data } = useQuery({
    queryKey: ['staff-name-map'],
    queryFn: async () => {
      const [staff, admins] = await Promise.all([
        getPlayers({ role: 'staff', limit: 200 }),
        getPlayers({ role: 'admin', limit: 50 }),
      ]);
      const map = new Map<string, string>();
      for (const user of [...staff.data, ...admins.data]) {
        map.set(user.id, displayName(user.firstName, user.lastName, user.username));
      }
      return map;
    },
    staleTime: 1000 * 60 * 5,
  });

  const resolveName = (userId: string) => data?.get(userId) ?? `${userId.slice(0, 8)}…`;

  return { resolveName, nameMap: data };
}
