import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { formatTimeAgo } from '@gaming-cafe/utils';
import { Pause, Timer, Visibility } from '@mui/icons-material';
import { Box, Chip, Pagination, Stack, Typography } from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { SessionRemainingClock } from '../../../components/SessionRemainingClock';
import { StaffTotpDialog } from '../../../components/StaffTotpDialog';
import { useSelector } from '../../../hooks/store';
import { useEnrichedSessions } from '../../../hooks/useEnrichedSessions';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getSessions, type SessionResponse } from '../../../services/sessions/list';
import { endSession } from '../../../services/sessions/update';
import { buildListUrl } from '../../../utils/buildListUrl';

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
    }, 1000);
    return () => clearInterval(interval);
  }, [startedAt]);

  const totalSeconds = Math.floor(timeElapsed / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  return (
    <Typography variant="body2">
      {` ${hours > 0 ? `${hours} hours` : ''} ${
        minutes > 0 ? `${minutes} min` : ''
      } ${seconds > 0 ? `${seconds} sec` : ''}`}
    </Typography>
  );
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
    refetchInterval: activeParam === 'true' ? 30_000 : false,
  });

  const enrichedSessions = useEnrichedSessions(data?.data);
  const currentUserRole = useSelector((state) => state.auth.role);
  const { can } = usePermissions();
  const queryClient = useQueryClient();
  const navigate = useNavigate();

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

  const requestEndSession = useCallback((id: string) => {
    if (currentUserRole === 'staff') {
      setTotpDialog({ open: true, sessionId: id, loading: false });
      return;
    }
    void (async () => {
      try {
        await endSession(id, { reason: 'force' });
        toast.success('Session ended successfully');
        void refetch();
        void queryClient.invalidateQueries({ queryKey: ['sessions'] });
      } catch {
        toast.error('Failed to end session');
      }
    })();
  }, [currentUserRole, refetch, queryClient]);

  const handleTotpConfirm = async (staffTotp: string) => {
    setTotpDialog((prev) => ({ ...prev, loading: true }));
    try {
      await endSession(totpDialog.sessionId, { staffTotp, reason: 'force' });
      toast.success('Session ended successfully');
      setTotpDialog({ open: false, sessionId: '', loading: false });
      void refetch();
      void queryClient.invalidateQueries({ queryKey: ['sessions'] });
    } catch {
      toast.error('Failed to end session');
      setTotpDialog((prev) => ({ ...prev, loading: false }));
    }
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
    const active =
      filter === 'active' ? 'true' : filter === 'completed' ? 'false' : undefined;
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
        label: 'Started',
        minWidth: 120,
        hideOnMobile: true,
        format: (value) => {
          const id = value as SessionResponse['id'];
          const session = enrichedSessions.find((s) => s.id === id);
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
        label: 'Time left',
        minWidth: 100,
        format: (value) => {
          const id = value as SessionResponse['id'];
          const session = enrichedSessions.find((s) => s.id === id);
          const startTime = session?.startTime;
          const endTime = session?.endTime;
          if (!session?.balance || !startTime) {
            if (endTime) return formatTimeAgo(endTime);
            return 'N/A';
          }
          if (endTime) return formatTimeAgo(endTime);
          return (
            <SessionRemainingClock
              remainingMinutes={session.balance.remainingMinutes}
              deductionProfile={session.balance.deductionProfile}
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
    [enrichedSessions],
  );

  const actions: Action<SessionResponse>[] = [
    {
      icon: <Pause color="error" />,
      label: 'End Session',
      onClick: (row) => requestEndSession(row.id),
      show: (row) => !row.endTime,
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
    <Box sx={{ px: 4, py: 2 }}>
      <Stack direction="row" spacing={1} sx={{ mb: 2, flexWrap: 'wrap', gap: 1 }}>
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

      <ListViewPage<SessionResponse>
        title="Sessions"
        description="Manage player gaming sessions and usage tracking."
        data={enrichedSessions}
        columns={columns}
        actions={actions}
        isLoading={isLoading}
        inputValue=""
        handleSearch={() => {}}
        handleClearSearch={() => {}}
        showSearch={false}
        onAddClick={can(Permission.SessionsWrite) ? handleStartNewSession : undefined}
        addButtonLabel="Start New Session"
      />
      <Box sx={{ display: 'flex', justifyContent: 'center', mt: 2 }}>
        <Pagination
          count={data?.totalPages}
          page={page}
          shape="rounded"
          hidePrevButton={page === 1}
          hideNextButton={page === data?.totalPages}
          onChange={(_event, value) =>
            navigate(
              buildListUrl('/sessions', value, {
                active: activeParam ?? undefined,
              }),
            )
          }
        />
      </Box>

      <StaffTotpDialog
        open={totpDialog.open}
        title="End session"
        description="Enter your authenticator code to end this session."
        confirmLabel="End session"
        loading={totpDialog.loading}
        onClose={() => setTotpDialog({ open: false, sessionId: '', loading: false })}
        onConfirm={handleTotpConfirm}
      />
    </Box>
  );
}
