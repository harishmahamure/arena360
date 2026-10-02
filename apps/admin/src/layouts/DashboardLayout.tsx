import { DEFAULT_CAFE_TZ, Permission } from '@gaming-cafe/contracts';
import { local, toastUtils } from '@gaming-cafe/utils';
import { LinearProgress } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { Suspense, useCallback, useEffect, useMemo, useState } from 'react';
import { Navigate, Outlet, useLocation, useNavigate } from 'react-router-dom';
import NotificationBell from '../components/notifications/NotificationBell';
import ShiftHandoverDialog from '../components/ShiftHandoverDialog';
import { adminNavItems } from '../constants/navItems';
import { useDispatch, useSelector } from '../hooks/store';
import { type CountdownConfig, useMultipleCountdowns } from '../hooks/useCountDown';
import { usePermissions } from '../hooks/usePermissions';
import { clearAdminSession } from '../lib/authSession';
import { getMyAccess } from '../services/access';
import { getSessions } from '../services/sessions/list';
import { getActiveShift } from '../services/shifts';
import { formatDuration, now } from '../utils/date';
import { filterNavItemsByPermission } from '../utils/filterNavItems';
import { getRouteTitle } from '../utils/routeTitle';
import { WorkspaceShell } from './WorkspaceShell';

export default function DashboardLayout() {
  const navigate = useNavigate();
  const dispatch = useDispatch();
  const location = useLocation();
  const outletKey = `${location.pathname}${location.search}`;

  const { email, firstName, lastName, username, role, avatarUrl } = useSelector(
    (state) => state.auth,
  );
  const { can, isStaff } = usePermissions();
  const [handoverOpen, setHandoverOpen] = useState(false);

  const accessToken = local.get('accessToken');
  const isAuthenticated = Boolean(accessToken && role);
  const { data: myAccess } = useQuery({
    queryKey: ['myAccess'],
    queryFn: getMyAccess,
    enabled: isAuthenticated,
  });

  const { data, isLoading } = useQuery({
    queryKey: ['sessions'],
    queryFn: () =>
      getSessions({
        isActive: 1,
      }),
    refetchInterval: false,
    enabled: isAuthenticated && can(Permission.SessionsRead),
  });

  const { data: activeShift } = useQuery({
    queryKey: ['activeShift'],
    queryFn: getActiveShift,
    retry: false,
    enabled: isAuthenticated && isStaff,
  });

  const sessions = data?.data ?? [];

  const countDownData = useMemo<CountdownConfig[]>(() => {
    if (!sessions.length || isLoading) return [];

    return sessions
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
  }, [sessions, isLoading]);

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
    if (isStaff && activeShift) {
      setHandoverOpen(true);
    } else {
      handleAdminLogout();
    }
  };

  const requireShiftForQuickAction = useCallback(
    (path: string) => {
      if (!activeShift) {
        toastUtils.warning('Confirm or resume your shift before using counter actions.');
        navigate('/shift/setup');
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
      onClick: () => navigate('/shift/setup'),
    };
  }, [activeShift, isStaff, navigate]);

  if (!isAuthenticated) {
    return <Navigate to="/login" replace />;
  }

  return (
    <>
      <WorkspaceShell
        navItems={filteredNavItems}
        pageTitle={pageTitle}
        shiftBadge={shiftBadge}
        user={{
          name: `${firstName} ${lastName}`.trim() || username,
          email,
          role: myAccess?.roles.join(', ') || 'Team member',
          avatarUrl,
        }}
        onLogout={handleLogout}
        appBarQuickActions={appBarQuickActions}
        settingsPath={can(Permission.SettingsRead) ? '/settings' : undefined}
        notificationSlot={can(Permission.NotificationsRead) ? <NotificationBell /> : undefined}
      >
        <Suspense key={location.pathname} fallback={<LinearProgress aria-label="Loading page" />}>
          <Outlet />
        </Suspense>
      </WorkspaceShell>
      {isStaff && (
        <ShiftHandoverDialog open={handoverOpen} onClose={() => setHandoverOpen(false)} />
      )}
    </>
  );
}
