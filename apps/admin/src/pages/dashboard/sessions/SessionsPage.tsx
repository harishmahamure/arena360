import { type Action, type Column, ListViewPage } from '@gaming-cafe/ui';
import { formatTimeAgo } from '@gaming-cafe/utils';
import { Pause, Timer, Visibility } from '@mui/icons-material';
import { Box, Chip, debounce, Pagination, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { memo, useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { toast } from 'react-toastify';
import { useEnrichedSessions } from '../../../hooks/useEnrichedSessions';
import { getSessions, type SessionResponse } from '../../../services/sessions/list';
import { endSession } from '../../../services/sessions/update';

const getStatusColor = (isActive: boolean) => {
  return isActive ? 'success' : 'default';
};

const formatDuration = (minutes?: number) => {
  if (!minutes) return 'N/A';
  const hours = Math.floor(minutes / 60);
  const mins = minutes % 60;
  return hours > 0 ? `${hours}h ${mins}m` : `${mins}m`;
};

const CountDownComponent = memo(
  ({
    remainingTime,
    handleTimeExpired,
  }: {
    remainingTime: number;
    handleTimeExpired?: () => void;
  }) => {
    const [timeLeft, setTimeLeft] = useState(Math.floor(remainingTime / 1000));
    const hasExpiredCallbackFired = useRef(false);

    useEffect(() => {
      hasExpiredCallbackFired.current = false;
    }, []);

    useEffect(() => {
      if (timeLeft <= 0) return;

      const interval = setInterval(() => {
        setTimeLeft((prev) => {
          const newValue = prev - 1;

          if (newValue <= 20 && !hasExpiredCallbackFired.current && handleTimeExpired) {
            hasExpiredCallbackFired.current = true;
            handleTimeExpired();
          }

          if (newValue <= 0) {
            clearInterval(interval);
            return 0;
          }

          return newValue;
        });
      }, 1000);

      return () => clearInterval(interval);
    }, [timeLeft, handleTimeExpired]);

    if (timeLeft <= 0) {
      return (
        <div>
          <Typography variant="body2" color="error">
            Expired
          </Typography>
        </div>
      );
    }

    const hours = Math.floor(timeLeft / 3600);
    const minutes = Math.floor((timeLeft % 3600) / 60);
    const seconds = timeLeft % 60;

    return (
      <div>
        <Typography variant="body2">
          {hours > 0
            ? `${hours} hours ${minutes} min ${seconds} sec`
            : `${minutes} min ${seconds} sec`}
        </Typography>
      </div>
    );
  },
);

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
  const [inputValue, setInputValue] = useState<string>('');
  const [debouncedSearch, setDebouncedSearch] = useState<string>('');
  const [searchParams] = useSearchParams();
  const page = Number(searchParams.get('page')) || 1;
  const activeFilter = searchParams.get('active') || undefined;

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['sessions', debouncedSearch, page, activeFilter],
    queryFn: () =>
      getSessions({
        page: page,
        isActive: activeFilter === 'true' ? 1 : 0,
      }),
  });

  const enrichedSessions = useEnrichedSessions(data?.data);

  const navigate = useNavigate();

  const debouncedSetSearch = useRef(
    debounce((query: string) => setDebouncedSearch(query), 500),
  ).current;

  const handleStartNewSession = useCallback(() => {
    navigate('/sessions/new');
  }, [navigate]);

  const handleViewSession = useCallback(
    (id: string) => {
      navigate(`/sessions/${id}`);
    },
    [navigate],
  );

  const handleEndSession = useCallback(
    async (id: string) => {
      try {
        await endSession(id);
        toast.success('Session ended successfully');
        // Refetch the data
        refetch();
      } catch (_error) {
        toast.error('Failed to end session');
      }
    },
    [refetch],
  );

  const handleSearch = useCallback(
    (event: React.ChangeEvent<HTMLInputElement>) => {
      const query = event.target.value;
      setInputValue(query);
      debouncedSetSearch(query);
    },
    [debouncedSetSearch],
  );

  const handleClearSearch = useCallback(() => {
    setInputValue('');
    setDebouncedSearch('');
    debouncedSetSearch.clear();
  }, [debouncedSetSearch]);

  const columns: Column<SessionResponse>[] = useMemo(
    () => [
      {
        id: 'playerPlan',
        label: 'Player',
        minWidth: 150,
        format: (value) => {
          const playerPlan = value as SessionResponse['playerPlan'];
          return playerPlan?.player?.username || 'N/A';
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
        format: (value) => {
          const id = value as SessionResponse['id'];
          const session = enrichedSessions.find((session) => session.id === id);
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
        label: 'Ending/Ended',
        minWidth: 100,
        format: (value) => {
          const id = value as SessionResponse['id'];
          const session = enrichedSessions.find((session) => session.id === id);
          const startTime = session?.startTime;
          const endTime = session?.endTime;
          const remainingTimeCredits = session?.playerPlan?.remainingTimeCredits ?? 0;

          if (!session?.playerPlan || !startTime) {
            if (endTime) {
              return formatTimeAgo(endTime);
            }
            return 'N/A';
          }

          const expectedEndTime = new Date(
            new Date(startTime || '').getTime() + remainingTimeCredits * 60 * 1000,
          );

          const remainingTime = expectedEndTime.getTime() - Date.now();

          if (endTime) {
            return formatTimeAgo(endTime || 'N/A');
          }
          if (remainingTime <= 0) {
            return 'Expired';
          }
          return (
            <CountDownComponent
              remainingTime={remainingTime}
              handleTimeExpired={() => handleEndSession(id)}
            />
          );
        },
      },
      {
        id: 'durationMinutes',
        label: 'Duration',
        minWidth: 100,
        align: 'right',
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
    [enrichedSessions, handleEndSession],
  );

  const handleIncreaseSessionTime = useCallback(() => {
    navigate('/plans/new');
  }, [navigate]);

  const actions: Action<SessionResponse>[] = [
    {
      icon: <Pause color="error" />,
      label: 'End Session',
      onClick: (row) => handleEndSession(row.id),
      show: (row) => !row.endTime, // Only show for active sessions
    },
    {
      icon: <Visibility color="info" />,
      label: 'View Session',
      onClick: (row) => handleViewSession(row.id),
    },
    {
      icon: <Timer color="primary" />,
      label: 'Increase Session Time Session',
      onClick: handleIncreaseSessionTime,
      show: (row) => !row.endTime, // Only show for active sessions
    },
  ];

  return (
    <Box sx={{ px: 4, py: 2 }}>
      <ListViewPage<SessionResponse>
        title="Usage Sessions"
        description="Manage player gaming sessions and usage tracking here."
        data={enrichedSessions}
        columns={columns}
        actions={actions}
        isLoading={isLoading}
        inputValue={inputValue}
        handleSearch={handleSearch}
        handleClearSearch={handleClearSearch}
        onAddClick={handleStartNewSession}
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
            navigate(value === 1 ? `/sessions` : `/sessions?page=${value}`)
          }
        />
      </Box>
    </Box>
  );
}
