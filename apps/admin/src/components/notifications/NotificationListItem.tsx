import { Box, Chip, ListItemButton, Typography } from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import { useNavigate } from 'react-router-dom';
import type { NotificationItem } from '../../services/notifications';
import {
  formatNotificationSummary,
  formatRelativeTime,
  formatSaleNotificationDetails,
  getNotificationLink,
  kindLabel,
} from './notificationUtils';

interface NotificationListItemProps {
  item: NotificationItem;
  onRead?: (id: string) => void;
}

export default function NotificationListItem({ item, onRead }: NotificationListItemProps) {
  const theme = useTheme();
  const navigate = useNavigate();
  const link = getNotificationLink(item.entityType, item.entityId, item.payload);
  const unread = !item.readAt;
  const saleDetails = formatSaleNotificationDetails(item.payload);
  const summary = saleDetails ?? (item.summary ? formatNotificationSummary(item.summary) : null);

  const handleClick = () => {
    if (unread && onRead) {
      onRead(item.id);
    }
    if (link) {
      navigate(link);
    }
  };

  return (
    <ListItemButton
      onClick={handleClick}
      sx={{
        alignItems: 'stretch',
        px: 2,
        py: 1.75,
        gap: 1.5,
        borderLeft: unread ? `3px solid ${theme.palette.primary.main}` : '3px solid transparent',
        bgcolor: unread ? alpha(theme.palette.primary.main, 0.08) : 'transparent',
        borderBottom: `1px solid ${alpha(theme.palette.divider, 0.6)}`,
        '&:hover': {
          bgcolor: unread
            ? alpha(theme.palette.primary.main, 0.12)
            : alpha(theme.palette.action.hover, 0.04),
        },
      }}
    >
      <Box sx={{ flex: 1, minWidth: 0 }}>
        <Box
          sx={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 1,
            mb: 0.75,
          }}
        >
          <Chip
            label={kindLabel(item.kind)}
            size="small"
            color={unread ? 'primary' : 'default'}
            variant={unread ? 'filled' : 'outlined'}
            sx={{
              height: 22,
              fontSize: '0.7rem',
              fontWeight: 600,
              '& .MuiChip-label': { px: 1 },
            }}
          />
          <Typography variant="caption" color="text.secondary" sx={{ flexShrink: 0 }}>
            {formatRelativeTime(item.createdAt)}
          </Typography>
        </Box>

        <Typography
          variant="body2"
          fontWeight={unread ? 600 : 500}
          color="text.primary"
          sx={{ lineHeight: 1.4, mb: summary ? 0.5 : 0 }}
        >
          {item.title}
        </Typography>

        {summary && (
          <Typography variant="body2" color="text.secondary" sx={{ lineHeight: 1.4 }}>
            {summary}
          </Typography>
        )}
      </Box>
    </ListItemButton>
  );
}
