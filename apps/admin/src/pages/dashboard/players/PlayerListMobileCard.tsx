import type { UserRole } from '@gaming-cafe/contracts';
import type { Action } from '@gaming-cafe/ui';
import { Avatar, Box, Button, Card, CardContent, Chip, Stack, Typography } from '@mui/material';
import type { PlayerResponse } from '../../../services/players/list';
import { formatDisplayDate } from '../../../utils/date';

const getRoleColor = (role: UserRole) => {
  switch (role) {
    case 'admin':
      return 'error';
    case 'staff':
      return 'warning';
    case 'player':
      return 'primary';
    default:
      return 'default';
  }
};

const getAvatarColor = (role: UserRole) => {
  switch (role) {
    case 'admin':
      return 'error.main';
    case 'staff':
      return 'warning.main';
    default:
      return 'primary.main';
  }
};

const getInitials = (firstName?: string, lastName?: string, username?: string) => {
  if (firstName && lastName) {
    return `${firstName.charAt(0)}${lastName.charAt(0)}`.toUpperCase();
  }
  if (firstName) {
    return firstName.charAt(0).toUpperCase();
  }
  if (username) {
    return username.charAt(0).toUpperCase();
  }
  return 'U';
};

interface PlayerListMobileCardProps {
  row: PlayerResponse;
  actions: Action<PlayerResponse>[];
}

export function PlayerListMobileCard({ row, actions }: PlayerListMobileCardProps) {
  const displayName =
    row.firstName && row.lastName ? `${row.firstName} ${row.lastName}` : row.username;
  const visibleActions = actions.filter((action) => action.show === undefined || action.show(row));

  return (
    <Card variant="outlined">
      <CardContent>
        <Stack spacing={1.5}>
          <Stack direction="row" spacing={1.5} alignItems="center">
            <Avatar
              sx={{
                bgcolor: getAvatarColor(row.role ?? 'player'),
                width: 40,
                height: 40,
              }}
            >
              {getInitials(row.firstName, row.lastName, row.username)}
            </Avatar>
            <Box sx={{ minWidth: 0, flex: 1 }}>
              <Typography variant="subtitle1" fontWeight={700} noWrap>
                {displayName}
              </Typography>
              <Typography variant="body2" color="text.secondary" noWrap>
                @{row.username}
              </Typography>
            </Box>
          </Stack>

          <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
            <Chip
              label={row.role.charAt(0).toUpperCase() + row.role.slice(1)}
              color={getRoleColor(row.role)}
              size="small"
              variant="outlined"
            />
            <Chip
              label={row.isActive ? 'Active' : 'Inactive'}
              color={row.isActive ? 'success' : 'default'}
              size="small"
            />
          </Stack>

          <Typography variant="body2" color="text.secondary">
            Joined {formatDisplayDate(row.createdAt)}
          </Typography>

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
