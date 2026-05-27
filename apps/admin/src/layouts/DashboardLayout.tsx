import { DashboardLayout as BaseDashboardLayout } from '@gaming-cafe/ui';
import { local } from '@gaming-cafe/utils';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';
import { Outlet, useNavigate } from 'react-router-dom';
import { adminNavItems } from '../constants/navItems';
import { useSelector } from '../hooks/store';
import { type CountdownConfig, useMultipleCountdowns } from '../hooks/useCountDown';
import { getSessions } from '../services/sessions/list';

export default function DashboardLayout() {
  const navigate = useNavigate();

  const { data, isLoading } = useQuery({
    queryKey: ['sessions'],
    queryFn: () =>
      getSessions({
        isActive: 1,
      }),
    refetchInterval: 60000, // Refetch every 60 seconds to update active sessions
  });

  const countDownData = useMemo<CountdownConfig[]>(() => {
    if (!data?.data || isLoading) return [];

    return data.data.map((session) => {
      const remainingTimeCredits = session.playerPlan.remainingTimeCredits;
      const startTime = new Date(session.startTime).getTime();
      const expectedEndTime = startTime + remainingTimeCredits * 60 * 1000;
      const remainingTime = (expectedEndTime - Date.now()) / 1000;

      return {
        id: session.id,
        remainingTime,
        sessionDetails: {
          playerName: session.playerPlan.player.username,
          deviceName: session.device.name,
        },
      };
    });
  }, [data?.data, isLoading]);

  useMultipleCountdowns(countDownData);

  const { email, firstName, lastName, role } = useSelector((state) => state.auth);

  useEffect(() => {
    const accessToken = local.get('accessToken');
    if (!accessToken) {
      navigate('/login');
    }
  }, [navigate]);

  return (
    <BaseDashboardLayout
      navItems={adminNavItems}
      user={{ name: `${firstName} ${lastName}`, email, role }}
    >
      <Outlet />
    </BaseDashboardLayout>
  );
}
