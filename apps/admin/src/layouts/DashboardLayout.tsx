import { DEFAULT_CAFE_TZ, Permission } from '@gaming-cafe/contracts';
import { BRAND_LOGO_URL } from '@gaming-cafe/theme';
import { DashboardLayout as BaseDashboardLayout } from '@gaming-cafe/ui';
import { local, toastUtils } from '@gaming-cafe/utils';
import Box from '@mui/material/Box';
import { useQuery } from '@tanstack/react-query';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { Navigate, Outlet, useLocation, useNavigate } from 'react-router-dom';
import ShiftHandoverDialog from '../components/ShiftHandoverDialog';
import { adminNavItems } from '../constants/navItems';
import { useDispatch, useSelector } from '../hooks/store';
import { type CountdownConfig, useMultipleCountdowns } from '../hooks/useCountDown';
import { useEnrichedSessions } from '../hooks/useEnrichedSessions';
import { usePermissions } from '../hooks/usePermissions';
import { clearAdminSession } from '../lib/authSession';
import { getSessions } from '../services/sessions/list';
import { getActiveShift } from '../services/shifts';
import { formatDuration, now } from '../utils/date';
import { filterNavItemsByPermission } from '../utils/filterNavItems';
import { getRouteTitle } from '../utils/routeTitle';

export default function DashboardLayout() {
  const navigate = useNavigate();
  const dispatch = useDispatch();
  const location = useLocation();
  const outletKey = `${location.pathname}${location.search}`;

  const { email, firstName, lastName, role } = useSelector((state) => state.auth);
  const { can, isStaff, isAdmin } = usePermissions();
  const [handoverOpen, setHandoverOpen] = useState(false);

  const accessToken = local.get('accessToken');
  const isAuthenticated = Boolean(accessToken && role);

  const { data, isLoading } = useQuery({
    queryKey: ['sessions'],
    queryFn: () =>
      getSessions({
        isActive: 1,
      }),
    refetchInterval: false,
    enabled: isAuthenticated,
  });

  const { data: activeShift } = useQuery({
    queryKey: ['activeShift'],
    queryFn: getActiveShift,
    retry: false,
    enabled: isAuthenticated && isStaff,
  });

  const enrichedSessions = useEnrichedSessions(data?.data);

  const countDownData = useMemo<CountdownConfig[]>(() => {
    if (!enrichedSessions.length || isLoading) return [];

    return enrichedSessions
      .filter((session) => session.balance?.remainingMinutes != null && !session.endTime)
      .map((session) => ({
        id: session.id,
        sessionStartTime: session.startTime,
        remainingMinutes: session.balance?.remainingMinutes ?? 0,
        timeCreditsConsumed: session.timeCreditsConsumed,
        deductionProfile: session.balance?.deductionProfile,
        cafeTimezone: session.cafeTimezone ?? DEFAULT_CAFE_TZ,
        expiryDate: session.balance?.expiryDate,
        sessionDetails: {
          playerName: session.balance?.player?.username ?? 'Unknown',
          deviceName: session.device?.name ?? 'Unknown',
        },
      }));
  }, [enrichedSessions, isLoading]);

  useMultipleCountdowns(countDownData);

  const filteredNavItems = useMemo(() => filterNavItemsByPermission(adminNavItems, can), [can]);

  useEffect(() => {
    if (!isAuthenticated) {
      navigate('/login', { replace: true });
    }
  }, [isAuthenticated, navigate]);

  useEffect(() => {
    if (outletKey) {
      window.scrollTo(0, 0);
    }
  }, [outletKey]);

  const handleAdminLogout = () => {
    clearAdminSession();
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

  const requireShiftForQuickAction = useCallback(
    (path: string) => {
      if (!activeShift) {
        toastUtils.warning('Start a shift from the dashboard before using this action.');
        navigate('/');
        return;
      }
      navigate(path);
    },
    [activeShift, navigate],
  );

  const appBarQuickActions = useMemo(
    () => ({
      showPos: isStaff && can(Permission.TransactionsWrite),
      showPlan: isStaff && can(Permission.PlayerPlansWrite),
      onPosClick: () => requireShiftForQuickAction('/product-transactions/new'),
      onPlanClick: () => requireShiftForQuickAction('/plan-transactions/new'),
    }),
    [can, isStaff, requireShiftForQuickAction],
  );

  const pageTitle = getRouteTitle(location.pathname);

  const shiftBadge = useMemo(() => {
    if (!isStaff) return undefined;
    if (activeShift) {
      const start = new Date(activeShift.clockIn);
      const diffMs = now().getTime() - start.getTime();
      const duration = formatDuration(diffMs / 60000);
      return {
        active: true,
        label: `Shift active • ${duration}`,
        onClick: () => navigate('/'),
      };
    }
    return {
      active: false,
      label: 'No active shift',
      onClick: () => navigate('/'),
    };
  }, [activeShift, isStaff, navigate]);

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  return (
    <>
      <BaseDashboardLayout
        navItems={filteredNavItems}
        pageTitle={pageTitle}
        shiftBadge={shiftBadge}
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
        appBarQuickActions={appBarQuickActions}
        settingsPath={isAdmin && can(Permission.ConfigRead) ? '/settings' : undefined}
      >
        <Outlet key={outletKey} />
      </BaseDashboardLayout>
      {isStaff && (
        <ShiftHandoverDialog open={handoverOpen} onClose={() => setHandoverOpen(false)} />
      )}
    </>
  );
}
