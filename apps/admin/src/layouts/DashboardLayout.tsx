import { BRAND_LOGO_URL } from '@gaming-cafe/theme';
import { DashboardLayout as BaseDashboardLayout } from '@gaming-cafe/ui';
import { local } from '@gaming-cafe/utils';
import Box from '@mui/material/Box';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { Outlet, useNavigate } from 'react-router-dom';
import ShiftHandoverDialog from '../components/ShiftHandoverDialog';
import { adminNavItems } from '../constants/navItems';
import { useSelector } from '../hooks/store';
import { type CountdownConfig, useMultipleCountdowns } from '../hooks/useCountDown';
import { usePermissions } from '../hooks/usePermissions';
import { getSessions } from '../services/sessions/list';
import { filterNavItemsByPermission } from '../utils/filterNavItems';

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

    return data.data
      .filter((session) => session.balance?.remainingMinutes != null)
      .map((session) => {
        const remainingMinutes = session.balance?.remainingMinutes ?? 0;
        const startTime = new Date(session.startTime).getTime();
        const expectedEndTime = startTime + remainingMinutes * 60 * 1000;
        const remainingTime = (expectedEndTime - Date.now()) / 1000;

        return {
          id: session.id,
          remainingTime,
          sessionDetails: {
            playerName: session.balance?.player?.username ?? 'Unknown',
            deviceName: session.device?.name ?? 'Unknown',
          },
        };
      });
  }, [data?.data, isLoading]);

  useMultipleCountdowns(countDownData);

  const { email, firstName, lastName, role } = useSelector((state) => state.auth);
  const { can, isStaff } = usePermissions();
  const [handoverOpen, setHandoverOpen] = useState(false);

  const filteredNavItems = useMemo(() => filterNavItemsByPermission(adminNavItems, can), [can]);

  useEffect(() => {
    const accessToken = local.get('accessToken');
    if (!accessToken) {
      navigate('/login');
    }
  }, [navigate]);

  return (
    <>
      <BaseDashboardLayout
        navItems={filteredNavItems}
        logo={
          <Box
            component="img"
            src={BRAND_LOGO_URL}
            alt="Arena360"
            sx={{ height: 40, width: 'auto', display: 'block' }}
          />
        }
        logoText="Arena360"
        user={{ name: `${firstName} ${lastName}`, email, role }}
        onLogout={isStaff ? () => setHandoverOpen(true) : undefined}
      >
        <Outlet />
      </BaseDashboardLayout>
      {isStaff && (
        <ShiftHandoverDialog open={handoverOpen} onClose={() => setHandoverOpen(false)} />
      )}
    </>
  );
}
