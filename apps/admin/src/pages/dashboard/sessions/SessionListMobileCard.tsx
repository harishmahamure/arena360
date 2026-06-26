import { DEFAULT_CAFE_TZ } from '@gaming-cafe/contracts';
import type { Action } from '@gaming-cafe/ui';
import { formatRemainingLabel, formatTimeAgo } from '@gaming-cafe/utils';
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
    if (row.endTime) {
      return (
        <Stack spacing={0.25}>
          <Typography variant="body2" color="text.secondary">
            Expired
          </Typography>
          <Typography variant="caption" color="text.secondary">
            Ended {formatTimeAgo(row.endTime)}
          </Typography>
        </Stack>
      );
    }
    if (!row.balance || !row.startTime) {
      return 'N/A';
    }
    return (
      <SessionRemainingClock
        sessionStartTime={row.startTime}
        remainingMinutes={row.balance.remainingMinutes}
        timeCreditsConsumed={row.timeCreditsConsumed}
        deductionProfile={row.balance.deductionProfile}
        cafeTimezone={row.cafeTimezone ?? DEFAULT_CAFE_TZ}
        expiryDate={row.balance.expiryDate}
      />
    );
  })();

  const visibleActions = actions.filter((action) => action.show === undefined || action.show(row));

  return (
    <Card variant="outlined">
      <CardContent>
        <Stack spacing={1.5}>
          <Stack direction="row" justifyContent="space-between" alignItems="flex-start" gap={1}>
            <Stack spacing={0.25} sx={{ flex: 1, minWidth: 0 }}>
              <Typography variant="subtitle1" fontWeight={700} noWrap>
                {playerName}
              </Typography>
              {row.walletMinutesAtStart != null && (
                <Typography variant="caption" color="text.secondary">
                  {formatRemainingLabel(row.walletMinutesAtStart)} at login
                </Typography>
              )}
            </Stack>
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
