import { DEFAULT_CAFE_TZ, SESSION_CLOCK_TICK_MS } from '@gaming-cafe/contracts';
import { type Action, type Column, ListPage } from '@gaming-cafe/ui';
import {
  formatRemainingClock,
  formatTimeAgo,
  toastUtils,
  useAsyncAction,
} from '@gaming-cafe/utils';
import { Pause, Timer, Visibility } from '@mui/icons-material';
import { Chip, Stack, Typography } from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { SessionRemainingClock } from '../../../components/SessionRemainingClock';
import { StaffTotpDialog } from '../../../components/StaffTotpDialog';
import { useSelector } from '../../../hooks/store';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getSessions, type SessionResponse } from '../../../services/sessions/list';
import { endSession } from '../../../services/sessions/update';
import { buildListUrl } from '../../../utils/buildListUrl';
import { SessionListMobileCard } from './SessionListMobileCard';

const getStatusColor = (isActive: boolean) => {
  return isActive ? 'success' : 'default';
};

const formatDuration = (minutes?: number) => {
  if (!minutes) return 'N/A';
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;
};

type SessionFilter = 'all' | 'active' | 'completed';

const CountTimeComponent = memo(({ startedAt }: { startedAt: string }) => {
  const [timeElapsed, setTimeElapsed] = useState(0);
  const startTimeRef = useRef(new Date(startedAt).getTime());

  useEffect(() => {
    startTimeRef.current = new Date(startedAt).getTime();
    setTimeElapsed(Date.now() - startTimeRef.current);
    const interval = setInterval(() => {
      setTimeElapsed(Date.now() - startTimeRef.current);
    }, SESSION_CLOCK_TICK_MS);
    return () => clearInterval(interval);
  }, [startedAt]);

  const elapsedMinutes = timeElapsed / 60_000;

  return <Typography variant="body2">{formatRemainingClock(elapsedMinutes)}</Typography>;
});

export default function SessionsPage() {
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const activeParam = searchParams.get('active');
  const sessionFilter: SessionFilter =
    activeParam === 'true' ? 'active' : activeParam === 'false' ? 'completed' : 'all';

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['sessions', page, activeParam],
    queryFn: () =>
      getSessions({
        page,
        ...(activeParam === 'true' ? { isActive: 1 } : {}),
        ...(activeParam === 'false' ? { isActive: 0 } : {}),
      }),
  });

  const sessions = data?.data ?? [];
  const currentUserRole = useSelector((state) => state.auth.role);
  const { can } = usePermissions();
  const queryClient = useQueryClient();
  const navigate = useNavigate();

  const { loading: endingSession, run: runEndSession } = useAsyncAction();

  const [totpDialog, setTotpDialog] = useState<{
    open: boolean;
    sessionId: string;
    loading: boolean;
  }>({ open: false, sessionId: '', loading: false });

  const handleStartNewSession = useCallback(() => {
    navigate('/sessions/new');
  }, [navigate]);

  const handleViewSession = useCallback(
    (id: string) => {
      navigate(`/sessions/${id}`);
    },
    [navigate],
  );

  const requestEndSession = useCallback(
    (id: string) => {
      if (currentUserRole === 'staff') {
        setTotpDialog({ open: true, sessionId: id, loading: false });
        return;
      }
      void runEndSession(async () => {
        try {
          await endSession(id, { reason: 'force' });
          toastUtils.success('Session ended successfully');
          void refetch();
          void queryClient.invalidateQueries({ queryKey: ['sessions'] });
        } catch {
          toastUtils.error('Failed to end session');
        }
      });
    },
    [currentUserRole, refetch, queryClient, runEndSession],
  );

  const handleTotpConfirm = (staffTotp: string) => {
    void runEndSession(async () => {
      setTotpDialog((prev) => ({ ...prev, loading: true }));
      try {
        await endSession(totpDialog.sessionId, { staffTotp, reason: 'force' });
        toastUtils.success('Session ended successfully');
        setTotpDialog({ open: false, sessionId: '', loading: false });
        void refetch();
        void queryClient.invalidateQueries({ queryKey: ['sessions'] });
      } catch {
        toastUtils.error('Failed to end session');
        setTotpDialog((prev) => ({ ...prev, loading: false }));
      }
    });
  };

  const handleBuyMoreTime = useCallback(
    (row: SessionResponse) => {
      const playerId = row.balance?.playerId;
      if (playerId) {
        navigate(`/plan-transactions/new?playerId=${playerId}`);
      } else {
        navigate('/plan-transactions/new');
      }
    },
    [navigate],
  );

  const setSessionFilter = (filter: SessionFilter) => {
    const active = filter === 'active' ? 'true' : filter === 'completed' ? 'false' : undefined;
    navigate(buildListUrl('/sessions', 1, { active }));
  };

  const columns: Column<SessionResponse>[] = useMemo(
    () => [
      {
        id: 'balance',
        label: 'Player',
        minWidth: 150,
        format: (value) => {
          const balance = value as SessionResponse['balance'];
          return balance?.player?.username || 'N/A';
        },
      },
      {
        id: 'device',
        label: 'Device',
        minWidth: 150,
        format: (value) => {
          const device = value as SessionResponse['device'];
          return device?.name || 'N/A';
        },
      },
      {
        id: 'id',
        key: 'started',
        label: 'Started',
        minWidth: 120,
        hideOnMobile: true,
        format: (value) => {
          const id = value as SessionResponse['id'];
          const session = sessions.find((s) => s.id === id);
          const startTime = session?.startTime;
          const endTime = session?.endTime;
          if (endTime) {
            return formatTimeAgo(startTime || '');
          }
          return <CountTimeComponent startedAt={startTime || ''} />;
        },
      },
      {
        id: 'id',
        key: 'time-left',
        label: 'Time left',
        minWidth: 100,
        format: (value) => {
          const id = value as SessionResponse['id'];
          const session = sessions.find((s) => s.id === id);
          const startTime = session?.startTime;
          const endTime = session?.endTime;
          if (endTime) {
            return (
              <Typography variant="body2" color="text.secondary">
                Expired
              </Typography>
            );
          }
          if (!session?.balance || !startTime) {
            return 'N/A';
          }
          return (
            <SessionRemainingClock
              sessionStartTime={startTime}
              remainingMinutes={session.balance.remainingMinutes}
              timeCreditsConsumed={session.timeCreditsConsumed}
              deductionProfile={session.balance.deductionProfile}
              cafeTimezone={session.cafeTimezone ?? DEFAULT_CAFE_TZ}
              expiryDate={session.balance.expiryDate}
            />
          );
        },
      },
      {
        id: 'durationMinutes',
        label: 'Duration',
        minWidth: 100,
        align: 'right',
        hideOnMobile: true,
        format: (value) => formatDuration(value as number),
      },
      {
        id: 'endTime',
        label: 'Status',
        minWidth: 100,
        align: 'center',
        format: (value) => (
          <Chip
            label={value ? 'Completed' : 'Active'}
            color={getStatusColor(!value)}
            size="small"
          />
        ),
      },
    ],
    [sessions],
  );

  const actions: Action<SessionResponse>[] = [
    {
      icon: <Pause color="error" />,
      label: 'End Session',
      onClick: (row) => requestEndSession(row.id),
      show: (row) => !row.endTime,
      disabled: () => endingSession,
    },
    {
      icon: <Visibility color="info" />,
      label: 'View Session',
      onClick: (row) => handleViewSession(row.id),
    },
    {
      icon: <Timer color="primary" />,
      label: 'Buy more time',
      onClick: (row) => handleBuyMoreTime(row),
      show: (row) => !row.endTime,
    },
  ];

  const filterChips: { key: SessionFilter; label: string }[] = [
    { key: 'all', label: 'All' },
    { key: 'active', label: 'Active' },
    { key: 'completed', label: 'Completed' },
  ];

  return (
    <>
      <ListPage<SessionResponse>
        title="Sessions"
        description="Manage player gaming sessions and usage tracking."
        data={sessions}
        columns={columns}
        actions={actions}
        isLoading={isLoading}
        filters={
          <Stack direction="row" spacing={1} sx={{ flexWrap: 'wrap', gap: 1 }}>
            {filterChips.map((chip) => (
              <Chip
                key={chip.key}
                label={chip.label}
                variant={sessionFilter === chip.key ? 'filled' : 'outlined'}
                color={sessionFilter === chip.key ? 'primary' : 'default'}
                onClick={() => setSessionFilter(chip.key)}
                clickable
              />
            ))}
          </Stack>
        }
        showSearch={false}
        onAddClick={can(Permission.SessionsWrite) ? handleStartNewSession : undefined}
        addButtonLabel="Start New Session"
        pagination={{
          page,
          totalPages: data?.totalPages,
          onPageChange: (value) =>
            navigate(
              buildListUrl('/sessions', value, {
                active: activeParam ?? undefined,
              }),
            ),
        }}
        mobileCardRender={(row, rowActions) => (
          <SessionListMobileCard row={row} actions={rowActions} />
        )}
      />

      <StaffTotpDialog
        open={totpDialog.open}
        title="End session"
        description="Enter your authenticator code to end this session."
        confirmLabel="End session"
        loading={totpDialog.loading}
        onClose={() => setTotpDialog({ open: false, sessionId: '', loading: false })}
        onConfirm={handleTotpConfirm}
      />
    </>
  );
}
