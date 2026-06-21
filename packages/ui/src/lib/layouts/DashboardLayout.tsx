'use client';

import { Box, useTheme } from '@mui/material';
import { useEffect, useState } from 'react';
import { useLocation, useSearchParams } from 'react-router-dom';
import AppBar, { type AppBarQuickActions, type ShiftBadgeProps } from '../components/AppBar';
import Sidebar, { type NavItem } from '../components/Sidebar';

const DRAWER_WIDTH = 280;
const COLLAPSED_WIDTH = 72;

export interface DashboardLayoutProps {
  children: React.ReactNode;
  navItems: NavItem[];
  logo?: React.ReactNode;
  logoText?: string;
  onLogout?: () => void;
  appBarQuickActions?: AppBarQuickActions;
  settingsPath?: string;
  pageTitle?: string;
  shiftBadge?: ShiftBadgeProps;
  notificationSlot?: React.ReactNode;
  user?: {
    name: string;
    email: string;
    role: string;
    avatar?: string;
  };
}

export default function DashboardLayout({
  children,
  navItems,
  logo,
  logoText,
  onLogout,
  appBarQuickActions,
  settingsPath,
  pageTitle,
  shiftBadge,
  notificationSlot,
  user,
}: DashboardLayoutProps) {
  const theme = useTheme();
  const [mobileOpen, setMobileOpen] = useState(false);
  const [collapsed, setCollapsed] = useState(false);
  const location = useLocation();

  const [_searchParams] = useSearchParams();
  const completePath = location.pathname;

  useEffect(() => {
    if (completePath === '/product-transactions/new') {
      setCollapsed(true);
    }
  }, [completePath]);

  const handleDrawerToggle = () => {
    setMobileOpen(!mobileOpen);
  };

  const handleCollapse = () => {
    setCollapsed(!collapsed);
  };

  return (
    <Box sx={{ display: 'flex', minHeight: '100vh' }}>
      <Sidebar
        open={mobileOpen}
        onClose={() => setMobileOpen(false)}
        collapsed={collapsed}
        onToggleCollapse={handleCollapse}
        navItems={navItems}
        logo={logo}
        logoText={logoText}
        user={user ? { name: user.name, role: user.role, avatar: user.avatar } : undefined}
        onLogout={onLogout}
      />

      <AppBar
        onMenuClick={handleDrawerToggle}
        sidebarCollapsed={collapsed}
        onLogout={onLogout}
        quickActions={appBarQuickActions}
        settingsPath={settingsPath}
        pageTitle={pageTitle}
        shiftBadge={shiftBadge}
        notificationSlot={notificationSlot}
        user={user ? { name: user.name, email: user.email, avatar: user.avatar } : undefined}
      />

      <Box
        component="main"
        sx={{
          flexGrow: 1,
          width: {
            xs: '100%',
            md: `calc(100% - ${collapsed ? COLLAPSED_WIDTH : DRAWER_WIDTH}px)`,
          },
          ml: { xs: 0, md: `${collapsed ? COLLAPSED_WIDTH : DRAWER_WIDTH}px` },
          mt: '64px',
          minHeight: 'calc(100vh - 64px)',
          bgcolor: 'background.default',
          transition: theme.transitions.create(['width', 'margin'], {
            easing: theme.transitions.easing.sharp,
            duration: theme.transitions.duration.enteringScreen,
          }),
        }}
      >
        {children}
      </Box>
    </Box>
  );
}
