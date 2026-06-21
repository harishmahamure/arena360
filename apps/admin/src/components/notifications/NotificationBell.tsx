import NotificationsNoneOutlinedIcon from '@mui/icons-material/NotificationsNoneOutlined';
import { Badge, IconButton, Tooltip } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { getUnreadCount } from '../../services/notifications';
import NotificationDrawer from './NotificationDrawer';

export default function NotificationBell() {
  const [drawerOpen, setDrawerOpen] = useState(false);

  const { data } = useQuery({
    queryKey: ['notifications', 'unread-count'],
    queryFn: getUnreadCount,
    refetchInterval: 60_000,
  });

  const unread = data?.count ?? 0;

  return (
    <>
      <Tooltip title="Notifications">
        <IconButton aria-label="Notifications" onClick={() => setDrawerOpen(true)}>
          <Badge badgeContent={unread} color="error" max={99}>
            <NotificationsNoneOutlinedIcon sx={{ color: 'text.secondary' }} />
          </Badge>
        </IconButton>
      </Tooltip>
      <NotificationDrawer open={drawerOpen} onClose={() => setDrawerOpen(false)} />
    </>
  );
}
