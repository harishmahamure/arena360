import CloseIcon from '@mui/icons-material/Close';
import { Box, Button, CircularProgress, Drawer, IconButton, List, Typography } from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useNavigate } from 'react-router-dom';
import {
  getNotifications,
  markAllNotificationsRead,
  markNotificationRead,
} from '../../services/notifications';
import NotificationListItem from './NotificationListItem';

interface NotificationDrawerProps {
  open: boolean;
  onClose: () => void;
}

export default function NotificationDrawer({ open, onClose }: NotificationDrawerProps) {
  const theme = useTheme();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const { data, isLoading } = useQuery({
    queryKey: ['notifications', { unreadOnly: false, limit: 10 }],
    queryFn: () => getNotifications({ limit: 10 }),
    enabled: open,
  });

  const markReadMutation = useMutation({
    mutationFn: markNotificationRead,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notifications'] });
    },
  });

  const markAllMutation = useMutation({
    mutationFn: markAllNotificationsRead,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['notifications'] });
    },
  });

  const items = data?.data ?? [];
  const hasUnread = items.some((item) => !item.readAt);

  return (
    <Drawer
      anchor="right"
      open={open}
      onClose={onClose}
      PaperProps={{
        sx: {
          width: { xs: '100%', sm: 400 },
          display: 'flex',
          flexDirection: 'column',
          bgcolor: 'background.default',
        },
      }}
    >
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'space-between',
          gap: 1,
          px: 2,
          py: 1.5,
          borderBottom: `1px solid ${alpha(theme.palette.divider, 0.6)}`,
        }}
      >
        <Typography variant="h6" fontWeight={600} color="text.primary">
          Notifications
        </Typography>
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 0.5 }}>
          {hasUnread && (
            <Button
              size="small"
              color="inherit"
              onClick={() => markAllMutation.mutate()}
              disabled={markAllMutation.isPending}
              sx={{ color: 'text.secondary', textTransform: 'none', minWidth: 'auto' }}
            >
              Mark all read
            </Button>
          )}
          <IconButton onClick={onClose} aria-label="Close notifications" size="small">
            <CloseIcon fontSize="small" />
          </IconButton>
        </Box>
      </Box>

      <Box sx={{ flex: 1, overflow: 'auto' }}>
        {isLoading ? (
          <Box sx={{ display: 'flex', justifyContent: 'center', py: 6 }}>
            <CircularProgress size={28} />
          </Box>
        ) : items.length === 0 ? (
          <Box sx={{ px: 3, py: 6, textAlign: 'center' }}>
            <Typography color="text.secondary" variant="body2">
              No notifications yet.
            </Typography>
            <Typography color="text.disabled" variant="caption" sx={{ mt: 0.5, display: 'block' }}>
              Important updates will appear here.
            </Typography>
          </Box>
        ) : (
          <List disablePadding>
            {items.map((item) => (
              <NotificationListItem
                key={item.id}
                item={item}
                onRead={(id) => markReadMutation.mutate(id)}
              />
            ))}
          </List>
        )}
      </Box>

      <Box
        sx={{
          p: 2,
          borderTop: `1px solid ${alpha(theme.palette.divider, 0.6)}`,
          bgcolor: 'background.paper',
        }}
      >
        <Button
          fullWidth
          variant="text"
          color="primary"
          onClick={() => {
            onClose();
            navigate('/activity-log');
          }}
          sx={{ textTransform: 'none', fontWeight: 600 }}
        >
          View activity log
        </Button>
      </Box>
    </Drawer>
  );
}
