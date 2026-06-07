import { BRAND_LOGO_URL } from '@gaming-cafe/theme';
import { DashboardLayout as BaseDashboardLayout } from '@gaming-cafe/ui';
import { local } from '@gaming-cafe/utils';
import Box from '@mui/material/Box';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { Outlet, useLocation, useNavigate } from 'react-router-dom';
import ShiftHandoverDialog from '../components/ShiftHandoverDialog';
import { adminNavItems } from '../constants/navItems';
import { useDispatch, useSelector } from '../hooks/store';
import { type CountdownConfig, useMultipleCountdowns } from '../hooks/useCountDown';
import { useEnrichedSessions } from '../hooks/useEnrichedSessions';
import { usePermissions } from '../hooks/usePermissions';
import { getSessions } from '../services/sessions/list';
import { filterNavItemsByPermission } from '../utils/filterNavItems';

export default function DashboardLayout() {
  const navigate = useNavigate();
  const dispatch = useDispatch();
  const location = useLocation();
  const outletKey = `${location.pathname}${location.search}`;

  const { data, isLoading } = useQuery({
    queryKey: ['sessions'],
    queryFn: () =>
      getSessions({
        isActive: 1,
      }),
    refetchInterval: 30_000,
  });

  const enrichedSessions = useEnrichedSessions(data?.data);

  const countDownData = useMemo<CountdownConfig[]>(() => {
    if (!enrichedSessions.length || isLoading) return [];

    return enrichedSessions
      .filter((session) => session.balance?.remainingMinutes != null && !session.endTime)
      .map((session) => ({
        id: session.id,
        remainingMinutes: session.balance?.remainingMinutes ?? 0,
        deductionProfile: session.balance?.deductionProfile,
        sessionDetails: {
          playerName: session.balance?.player?.username ?? 'Unknown',
          deviceName: session.device?.name ?? 'Unknown',
        },
      }));
  }, [enrichedSessions, isLoading]);

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

  useEffect(() => {
    if (outletKey) {
      window.scrollTo(0, 0);
    }
  }, [outletKey]);

  const handleAdminLogout = () => {
    local.remove('accessToken');
    dispatch({ type: 'Reset' });
    navigate('/login');
  };

  const handleLogout = () => {
    if (isStaff) {
      setHandoverOpen(true);
    } else {
      handleAdminLogout();
    }
  };

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
        onLogout={handleLogout}
      >
        <Outlet key={outletKey} />
      </BaseDashboardLayout>
      {isStaff && (
        <ShiftHandoverDialog open={handoverOpen} onClose={() => setHandoverOpen(false)} />
      )}
    </>
  );
}
