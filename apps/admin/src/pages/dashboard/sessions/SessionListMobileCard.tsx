import type { Action } from '@gaming-cafe/ui';
import { formatTimeAgo } from '@gaming-cafe/utils';
import { Box, Button, Card, CardContent, Chip, Stack, Typography } from '@mui/material';
import { SessionRemainingClock } from '../../../components/SessionRemainingClock';
import type { SessionResponse } from '../../../services/sessions/list';

interface SessionListMobileCardProps {
  row: SessionResponse;
  actions: Action<SessionResponse>[];
}

export function SessionListMobileCard({ row, actions }: SessionListMobileCardProps) {
  const playerName = row.balance?.player?.username ?? 'N/A';
  const deviceName = row.device?.name ?? 'N/A';
  const isActive = !row.endTime;

  const timeContent = (() => {
    if (!row.balance || !row.startTime) {
      if (row.endTime) return formatTimeAgo(row.endTime);
      return 'N/A';
    }
    if (row.endTime) return formatTimeAgo(row.endTime);
    return (
      <SessionRemainingClock
        remainingMinutes={row.balance.remainingMinutes}
        deductionProfile={row.balance.deductionProfile}
      />
    );
  })();

  const visibleActions = actions.filter((action) => action.show === undefined || action.show(row));

  return (
    <Card variant="outlined">
      <CardContent>
        <Stack spacing={1.5}>
          <Stack direction="row" justifyContent="space-between" alignItems="center" gap={1}>
            <Typography variant="subtitle1" fontWeight={700} noWrap sx={{ flex: 1 }}>
              {playerName}
            </Typography>
            <Chip
              label={isActive ? 'Active' : 'Completed'}
              color={isActive ? 'success' : 'default'}
              size="small"
            />
          </Stack>

          <Typography variant="body2" color="text.secondary">
            {deviceName}
          </Typography>

          <Box>{timeContent}</Box>

          {visibleActions.length > 0 && (
            <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
              {visibleActions.map((action) => {
                const isDisabled = action.disabled?.(row);
                return (
                  <Button
                    key={action.label}
                    variant="outlined"
                    size="small"
                    color={action.color === 'default' ? 'inherit' : action.color}
                    startIcon={action.icon}
                    disabled={isDisabled}
                    onClick={() => action.onClick(row)}
                    sx={{ minHeight: 44 }}
                  >
                    {action.label}
                  </Button>
                );
              })}
            </Stack>
          )}
        </Stack>
      </CardContent>
    </Card>
  );
}
