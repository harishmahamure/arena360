import { toastUtils } from '@gaming-cafe/utils';
import {
  LightMode,
  Logout,
  Menu as MenuIcon,
  PointOfSale,
  Receipt,
  Search,
  Settings,
} from '@mui/icons-material';
import {
  Avatar,
  Box,
  Chip,
  Divider,
  IconButton,
  InputAdornment,
  ListItemIcon,
  Menu,
  MenuItem,
  AppBar as MuiAppBar,
  TextField,
  Toolbar,
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

export interface AppBarQuickActions {
  showPos?: boolean;
  showPlan?: boolean;
  onPosClick?: () => void;
  onPlanClick?: () => void;
}

export interface ShiftBadgeProps {
  active: boolean;
  label: string;
  onClick?: () => void;
}

export interface AppBarProps {
  onMenuClick: () => void;
  sidebarCollapsed: boolean;
  drawerWidth?: number;
  collapsedWidth?: number;
  onLogout?: () => void;
  showGlobalSearch?: boolean;
  showThemeToggle?: boolean;
  quickActions?: AppBarQuickActions;
  settingsPath?: string;
  pageTitle?: string;
  shiftBadge?: ShiftBadgeProps;
  notificationSlot?: React.ReactNode;
  user?: {
    name: string;
    email: string;
    avatar?: string;
  };
}

const DRAWER_WIDTH = 280;
const COLLAPSED_WIDTH = 72;

export default function AppBar({
  onMenuClick,
  sidebarCollapsed,
  drawerWidth = DRAWER_WIDTH,
  collapsedWidth = COLLAPSED_WIDTH,
  onLogout,
  showGlobalSearch = false,
  showThemeToggle = false,
  quickActions,
  settingsPath,
  pageTitle,
  shiftBadge,
  notificationSlot,
  user = { name: 'John Doe', email: 'john.doe@example.com' },
}: AppBarProps) {
  const navigate = useNavigate();
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const [anchorEl, setAnchorEl] = useState<null | HTMLElement>(null);

  const handleProfileMenu = (event: React.MouseEvent<HTMLElement>) => {
    setAnchorEl(event.currentTarget);
  };

  const handleClose = () => {
    setAnchorEl(null);
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
        <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, minWidth: 0, flex: 1 }}>
          {isMobile && (
            <IconButton
              edge="start"
              color="inherit"
              onClick={onMenuClick}
              sx={{ color: 'text.primary', flexShrink: 0 }}
            >
              <MenuIcon />
            </IconButton>
          )}

          {pageTitle && (
            <Typography
              variant="h6"
              fontWeight={600}
              noWrap
              sx={{ color: 'text.primary', display: { xs: 'none', sm: 'block' } }}
            >
              {pageTitle}
            </Typography>
          )}

          {pageTitle && isMobile && (
            <Typography variant="subtitle1" fontWeight={600} noWrap sx={{ color: 'text.primary' }}>
              {pageTitle}
            </Typography>
          )}

          {shiftBadge && (
            <Chip
              size="small"
              label={shiftBadge.label}
              color={shiftBadge.active ? 'success' : 'warning'}
              variant={shiftBadge.active ? 'filled' : 'outlined'}
              onClick={shiftBadge.onClick}
              sx={{ cursor: shiftBadge.onClick ? 'pointer' : 'default', flexShrink: 0 }}
            />
          )}

          {showGlobalSearch && (
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
          )}
        </Box>

        <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexShrink: 0 }}>
          {showThemeToggle && (
            <IconButton sx={{ display: { xs: 'none', sm: 'flex' } }}>
              <LightMode sx={{ color: 'text.secondary' }} />
            </IconButton>
          )}

          {quickActions?.showPos && (
            <Tooltip title="POS — sell items">
              <IconButton aria-label="Open POS" onClick={quickActions.onPosClick}>
                <PointOfSale sx={{ color: 'text.secondary' }} />
              </IconButton>
            </Tooltip>
          )}

          {quickActions?.showPlan && (
            <Tooltip title="Buy plan">
              <IconButton aria-label="Buy plan" onClick={quickActions.onPlanClick}>
                <Receipt sx={{ color: 'text.secondary' }} />
              </IconButton>
            </Tooltip>
          )}

          {notificationSlot}

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

        <Menu
          anchorEl={anchorEl}
          open={Boolean(anchorEl)}
          onClose={handleClose}
          onClick={handleClose}
          transformOrigin={{ horizontal: 'right', vertical: 'top' }}
          anchorOrigin={{ horizontal: 'right', vertical: 'bottom' }}
          slotProps={{
            paper: { sx: { width: 220, mt: 1.5 } },
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
          {settingsPath && (
            <MenuItem onClick={() => navigate(settingsPath)}>
              <ListItemIcon>
                <Settings fontSize="small" />
              </ListItemIcon>
              Settings
            </MenuItem>
          )}
          <Divider />
          <MenuItem onClick={handleLogout} sx={{ color: 'error.main' }}>
            <ListItemIcon>
              <Logout fontSize="small" sx={{ color: 'error.main' }} />
            </ListItemIcon>
            Logout
          </MenuItem>
        </Menu>
      </Toolbar>
    </MuiAppBar>
  );
}
