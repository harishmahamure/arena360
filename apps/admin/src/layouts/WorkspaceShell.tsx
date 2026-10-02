import type { NavItem } from '@gaming-cafe/ui';
import {
  ArrowForward,
  Close,
  ExpandLess,
  ExpandMore,
  GridViewRounded,
  Logout,
  MenuRounded,
  Search,
  SettingsOutlined,
} from '@mui/icons-material';
import {
  Avatar,
  Box,
  Button,
  Chip,
  Dialog,
  DialogContent,
  DialogTitle,
  Drawer,
  IconButton,
  InputAdornment,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { type ReactNode, useEffect, useMemo, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { useRealtimeStatus } from '../lib/realtime/RealtimeProvider';
import AppearanceDialog from '../theme/AppearanceDialog';

export interface WorkspaceShellProps {
  children: ReactNode;
  navItems: NavItem[];
  pageTitle: string;
  user: { name: string; email: string; role: string };
  onLogout: () => void;
  shiftBadge?: { active: boolean; label: string; onClick: () => void };
  appBarQuickActions: {
    showPos: boolean;
    showPlan: boolean;
    onPosClick: () => void;
    onPlanClick: () => void;
  };
  settingsPath?: string;
  notificationSlot?: ReactNode;
}

export function WorkspaceShell({
  children,
  navItems,
  pageTitle,
  user,
  onLogout,
  shiftBadge,
  appBarQuickActions,
  settingsPath,
  notificationSlot,
}: WorkspaceShellProps) {
  const realtimeStatus = useRealtimeStatus();
  const location = useLocation();
  const navigate = useNavigate();
  const [mobileOpen, setMobileOpen] = useState(false);
  const [searchOpen, setSearchOpen] = useState(false);
  const [search, setSearch] = useState('');
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const current = useMemo(
    () =>
      navItems
        .filter(
          (item) =>
            item.path !== '/' &&
            (location.pathname === item.path ||
              location.pathname.startsWith(`${item.path}/`) ||
              item.children?.some((child) => child.path.split('?')[0] === location.pathname)),
        )
        .sort((a, b) => b.path.length - a.path.length)[0] ??
      navItems.find((item) => item.path === '/'),
    [navItems, location.pathname],
  );
  const groups = [...new Set(navItems.map((item) => item.section ?? 'Workspace'))];
  const destinations = navItems
    .flatMap((item) => [
      { title: item.title, path: item.path, section: item.section, icon: item.icon },
      ...(item.children ?? [])
        .filter((child) => child.path !== item.path)
        .map((child) => ({
          title: child.title,
          path: child.path,
          section: item.title,
          icon: item.icon,
        })),
    ])
    .filter((item) =>
      `${item.title} ${item.section} ${item.path} ${item.path === '/settings' ? 'pricing policies settings configuration' : ''}`
        .toLowerCase()
        .includes(search.toLowerCase()),
    );

  useEffect(() => {
    const handleKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'k') {
        event.preventDefault();
        setSearchOpen((open) => !open);
      }
    };
    window.addEventListener('keydown', handleKey);
    return () => window.removeEventListener('keydown', handleKey);
  }, []);
  // biome-ignore lint/correctness/useExhaustiveDependencies: Close the drawer when navigating between filters of the same page.
  useEffect(() => {
    document.title = `${pageTitle} · Arena360`;
    setMobileOpen(false);
  }, [location.pathname, location.search, pageTitle]);

  const sidebar = (
    <Box className="workspace-sidebar">
      <Link className="workspace-brand" to="/" aria-label="Arena360 home">
        <span className="brand-mark">
          <GridViewRounded />
        </span>
        <span>
          arena<span className="brand-number">360</span>
          <small>OPERATIONS WORKSPACE</small>
        </span>
      </Link>
      <Box className="workspace-context">
        <span className="workspace-context-icon" style={{ flexShrink: 0 }}>
          A
        </span>
        <div style={{ minWidth: 0 }}>
          <strong>Arena360</strong>
          <small
            title={user.role}
            style={{ overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
          >
            {user.role}
          </small>
        </div>
      </Box>
      <Button className="sidebar-search" startIcon={<Search />} onClick={() => setSearchOpen(true)}>
        Find a page <kbd>⌘ K</kbd>
      </Button>
      <Box component="nav" aria-label="Main navigation" className="workspace-nav">
        {groups.map((section) => (
          <Box key={section} className="nav-group">
            <Typography component="h2" className="nav-group-label">
              {section}
            </Typography>
            {navItems
              .filter((item) => (item.section ?? 'Workspace') === section)
              .map((item) => {
                const active = current?.path === item.path;
                const isOpen = expanded[item.path] ?? active;
                return (
                  <Box key={item.path}>
                    <Box className={`nav-item-row ${active ? 'is-active' : ''}`}>
                      <ListItemButton
                        component={Link}
                        to={item.path}
                        selected={active}
                        aria-current={active ? 'page' : undefined}
                      >
                        <ListItemIcon>{item.icon}</ListItemIcon>
                        <ListItemText primary={item.title} />
                      </ListItemButton>
                      {!!item.children?.length && (
                        <IconButton
                          size="small"
                          aria-label={`${isOpen ? 'Collapse' : 'Expand'} ${item.title}`}
                          aria-expanded={isOpen}
                          onClick={() =>
                            setExpanded((state) => ({ ...state, [item.path]: !isOpen }))
                          }
                        >
                          {isOpen ? (
                            <ExpandLess fontSize="small" />
                          ) : (
                            <ExpandMore fontSize="small" />
                          )}
                        </IconButton>
                      )}
                    </Box>
                    {isOpen && item.children && (
                      <Box className="nav-children">
                        {item.children.map((child) => (
                          <ListItemButton
                            key={child.path}
                            component={Link}
                            to={child.path}
                            selected={`${location.pathname}${location.search}` === child.path}
                          >
                            <ListItemText primary={child.title} />
                          </ListItemButton>
                        ))}
                      </Box>
                    )}
                  </Box>
                );
              })}
          </Box>
        ))}
      </Box>
      <Box className="sidebar-footer">
        <Avatar sx={{ width: 32, height: 32, bgcolor: '#31493f', fontSize: '0.8125rem' }}>
          {(user.name.trim() || user.email || 'A').slice(0, 1).toUpperCase()}
        </Avatar>
        <Box sx={{ flex: 1, minWidth: 0 }}>
          <Typography noWrap fontWeight={600} fontSize="0.75rem">
            {user.name.trim() || user.email || 'Account'}
          </Typography>
          <Typography fontSize="0.6875rem" color="#91a39b">
            {user.role}
          </Typography>
        </Box>
        <Tooltip title={shiftBadge?.active ? 'End shift and sign out' : 'Sign out'}>
          <IconButton onClick={onLogout} aria-label="Sign out" sx={{ color: '#a9bab3' }}>
            <Logout fontSize="small" />
          </IconButton>
        </Tooltip>
      </Box>
    </Box>
  );

  return (
    <Box className="workspace">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>
      <Box className="desktop-sidebar">{sidebar}</Box>
      <Drawer
        open={mobileOpen}
        onClose={() => setMobileOpen(false)}
        sx={{ '& .MuiDrawer-paper': { width: 256, border: 0 } }}
      >
        {sidebar}
      </Drawer>
      <Box className="workspace-body">
        <Box component="header" className="workspace-topbar">
          <IconButton
            className="mobile-menu"
            onClick={() => setMobileOpen(true)}
            aria-label="Open navigation"
          >
            <MenuRounded />
          </IconButton>
          <Box className="workspace-breadcrumb">
            <span>{current?.section ?? 'Workspace'}</span>
            <span>/</span>
            <strong>{pageTitle}</strong>
          </Box>
          <Box sx={{ flex: 1 }} />
          {shiftBadge && (
            <Chip
              label={shiftBadge.label}
              color={shiftBadge.active ? 'success' : 'warning'}
              onClick={shiftBadge.onClick}
            />
          )}
          {appBarQuickActions.showPos && (
            <Button onClick={appBarQuickActions.onPosClick} variant="contained" size="small">
              New sale
            </Button>
          )}
          {appBarQuickActions.showPlan && (
            <Button onClick={appBarQuickActions.onPlanClick} variant="outlined" size="small">
              Sell plan
            </Button>
          )}
          <Tooltip title="Search pages (⌘/Ctrl K)">
            <IconButton aria-label="Search pages" onClick={() => setSearchOpen(true)}>
              <Search fontSize="small" />
            </IconButton>
          </Tooltip>
          <AppearanceDialog />
          {notificationSlot}
          {settingsPath && (
            <Tooltip title="Configuration">
              <IconButton component={Link} to={settingsPath} aria-label="Open configuration">
                <SettingsOutlined fontSize="small" />
              </IconButton>
            </Tooltip>
          )}
          <Avatar
            sx={{
              width: 30,
              height: 30,
              fontSize: '0.75rem',
              bgcolor: '#e9eee9',
              color: '#315440',
            }}
          >
            {(user.name.trim() || 'A').slice(0, 1)}
          </Avatar>
        </Box>
        <Box component="main" id="main-content" tabIndex={-1} className="workspace-content">
          {children}
        </Box>
        <Box component="footer" className="workspace-footer">
          <span role="status">
            Arena360 ·{' '}
            {realtimeStatus === 'connected'
              ? 'Live updates connected'
              : realtimeStatus === 'connecting'
                ? 'Connecting to live updates…'
                : 'Live updates offline — reconnecting automatically'}
          </span>
          <span>{'Team workspace'}</span>
        </Box>
      </Box>
      <Dialog open={searchOpen} onClose={() => setSearchOpen(false)} fullWidth maxWidth="sm">
        <DialogTitle
          sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}
        >
          Go to a page
          <IconButton aria-label="Close page search" onClick={() => setSearchOpen(false)}>
            <Close />
          </IconButton>
        </DialogTitle>
        <DialogContent>
          <TextField
            autoFocus
            fullWidth
            label="Search pages and actions"
            value={search}
            onChange={(event) => setSearch(event.target.value)}
            slotProps={{
              input: {
                startAdornment: (
                  <InputAdornment position="start">
                    <Search />
                  </InputAdornment>
                ),
              },
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && destinations[0]) {
                navigate(destinations[0].path);
                setSearchOpen(false);
                setSearch('');
              }
            }}
          />
          <List sx={{ mt: 1, maxHeight: 360, overflow: 'auto' }} aria-label="Page search results">
            {destinations.map((item) => (
              <ListItemButton
                key={item.path}
                onClick={() => {
                  navigate(item.path);
                  setSearchOpen(false);
                  setSearch('');
                }}
              >
                <ListItemIcon>{item.icon}</ListItemIcon>
                <ListItemText primary={item.title} secondary={item.section} />
                <ArrowForward fontSize="small" color="action" />
              </ListItemButton>
            ))}
          </List>
          {destinations.length === 0 && (
            <Typography sx={{ py: 4, textAlign: 'center' }} color="text.secondary">
              No pages found. Try “sessions”, “pricing”, or “players”.
            </Typography>
          )}
        </DialogContent>
      </Dialog>
    </Box>
  );
}
