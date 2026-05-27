import { toastUtils } from '@gaming-cafe/utils';
import {
  LightMode,
  Logout,
  Menu as MenuIcon,
  Notifications,
  Person,
  Search,
  Settings,
} from '@mui/icons-material';
import {
  Avatar,
  Badge,
  Box,
  Divider,
  IconButton,
  InputAdornment,
  ListItemIcon,
  Menu,
  MenuItem,
  AppBar as MuiAppBar,
  TextField,
  Toolbar,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

export interface AppBarProps {
  onMenuClick: () => void;
  sidebarCollapsed: boolean;
  drawerWidth?: number;
  collapsedWidth?: number;
  onLogout?: () => void;
  user?: {
    name: string;
    email: string;
    avatar?: string;
  };
  notifications?: Array<{
    id: number;
    text: string;
    time: string;
  }>;
}

const DRAWER_WIDTH = 280;
const COLLAPSED_WIDTH = 72;

export default function AppBar({
  onMenuClick,
  sidebarCollapsed,
  drawerWidth = DRAWER_WIDTH,
  collapsedWidth = COLLAPSED_WIDTH,
  onLogout,
  user = { name: 'John Doe', email: 'john.doe@example.com' },
  notifications: propNotifications,
}: AppBarProps) {
  const navigate = useNavigate();
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);
  const [notificationAnchor, setNotificationAnchor] = useState<null | HTMLElement>(null);

  const handleProfileMenu = (event: React.MouseEvent<HTMLElement>) => {
    setAnchorEl(event.currentTarget);
  };

  const handleNotificationMenu = (event: React.MouseEvent<HTMLElement>) => {
    setNotificationAnchor(event.currentTarget);
  };

  const handleClose = () => {
    setAnchorEl(null);
    setNotificationAnchor(null);
  };

  const handleLogout = () => {
    if (onLogout) {
      onLogout();
      handleClose();
      return;
    }
    localStorage.removeItem('accessToken');
    toastUtils.success('Logged out successfully!');
    navigate('/login');
  };

  const notifications = propNotifications || [
    { id: 1, text: 'New order received #1234', time: '2 min ago' },
    { id: 2, text: 'Payment confirmed for #1232', time: '15 min ago' },
    { id: 3, text: 'New customer registered', time: '1 hour ago' },
    { id: 4, text: 'Stock alert: Product running low', time: '2 hours ago' },
  ];

  const userInitials = user.name
    .split(' ')
    .map((n) => n[0])
    .join('');

  return (
    <MuiAppBar
      position="fixed"
      sx={{
        width: {
          xs: '100%',
          md: `calc(100% - ${sidebarCollapsed ? collapsedWidth : drawerWidth}px)`,
        },
        ml: {
          xs: 0,
          md: `${sidebarCollapsed ? collapsedWidth : drawerWidth}px`,
        },
        transition: theme.transitions.create(['width', 'margin'], {
          easing: theme.transitions.easing.sharp,
          duration: theme.transitions.duration.enteringScreen,
        }),
      }}
    >
      <Toolbar
        sx={{
          justifyContent: 'space-between',
          gap: 2,
        }}
      >
        {/* Left Section */}
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2 }}>
          {isMobile && (
            <IconButton
              edge="start"
              color="inherit"
              onClick={onMenuClick}
              sx={{ color: 'text.primary' }}
            >
              <MenuIcon />
            </IconButton>
          )}

          {/* Search */}
          <TextField
            placeholder="Search..."
            size="small"
            sx={{
              width: { xs: 150, sm: 250, md: 350 },
              '& .MuiOutlinedInput-root': {
                bgcolor: 'background.default',
              },
            }}
            InputProps={{
              startAdornment: (
                <InputAdornment position="start">
                  <Search sx={{ color: 'text.secondary', fontSize: 20 }} />
                </InputAdornment>
              ),
            }}
          />
        </Box>

        {/* Right Section */}
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
          {/* Theme Toggle */}
          <IconButton sx={{ display: { xs: 'none', sm: 'flex' } }}>
            <LightMode sx={{ color: 'text.secondary' }} />
          </IconButton>

          {/* Notifications */}
          <IconButton onClick={handleNotificationMenu}>
            <Badge badgeContent={notifications.length} color="error">
              <Notifications sx={{ color: 'text.secondary' }} />
            </Badge>
          </IconButton>

          {/* Settings */}
          <IconButton sx={{ display: { xs: 'none', sm: 'flex' } }}>
            <Settings sx={{ color: 'text.secondary' }} />
          </IconButton>

          {/* Profile */}
          <IconButton onClick={handleProfileMenu} sx={{ p: 0, ml: 1 }}>
            <Avatar
              sx={{
                bgcolor: 'primary.main',
                width: 36,
                height: 36,
                fontSize: '0.875rem',
              }}
            >
              {userInitials}
            </Avatar>
          </IconButton>
        </Box>

        {/* Profile Menu */}
        <Menu
          anchorEl={anchorEl}
          open={Boolean(anchorEl)}
          onClose={handleClose}
          onClick={handleClose}
          transformOrigin={{ horizontal: 'right', vertical: 'top' }}
          anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
          PaperProps={{
            sx: { width: 220, mt: 1.5 },
          }}
        >
          <Box sx={{ px: 2, py: 1.5 }}>
            <Typography variant="subtitle2" fontWeight={600}>
              {user.name}
            </Typography>
            <Typography variant="caption" color="text.secondary">
              {user.email}
            </Typography>
          </Box>
          <Divider />
          <MenuItem>
            <ListItemIcon>
              <Person fontSize="small" />
            </ListItemIcon>
            Profile
          </MenuItem>
          <MenuItem>
            <ListItemIcon>
              <Settings fontSize="small" />
            </ListItemIcon>
            Settings
          </MenuItem>
          <Divider />
          <MenuItem onClick={handleLogout} sx={{ color: 'error.main' }}>
            <ListItemIcon>
              <Logout fontSize="small" sx={{ color: 'error.main' }} />
            </ListItemIcon>
            Logout
          </MenuItem>
        </Menu>

        {/* Notifications Menu */}
        <Menu
          anchorEl={notificationAnchor}
          open={Boolean(notificationAnchor)}
          onClose={handleClose}
          transformOrigin={{ horizontal: 'right', vertical: 'top' }}
          anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
          PaperProps={{
            sx: { width: 320, maxHeight: 400, mt: 1.5 },
          }}
        >
          <Box
            sx={{
              px: 2,
              py: 1.5,
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <Typography variant="subtitle2" fontWeight={600}>
              Notifications
            </Typography>
            <Typography variant="caption" color="primary.main" sx={{ cursor: 'pointer' }}>
              Mark all as read
            </Typography>
          </Box>
          <Divider />
          {notifications.map((notification) => (
            <MenuItem
              key={notification.id}
              onClick={handleClose}
              sx={{ py: 1.5, whiteSpace: 'normal' }}
            >
              <Box>
                <Typography variant="body2">{notification.text}</Typography>
                <Typography variant="caption" color="text.secondary">
                  {notification.time}
                </Typography>
              </Box>
            </MenuItem>
          ))}
          <Divider />
          <MenuItem sx={{ justifyContent: 'center', color: 'primary.main' }}>
            View all notifications
          </MenuItem>
        </Menu>
      </Toolbar>
    </MuiAppBar>
  );
}
