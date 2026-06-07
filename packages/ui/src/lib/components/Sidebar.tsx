import { ChevronLeft, ChevronRight, ExpandLess, ExpandMore, Logout } from '@mui/icons-material';
import {
  Avatar,
  Box,
  Collapse,
  Divider,
  Drawer,
  IconButton,
  List,
  ListItemButton,
  ListItemIcon,
  ListItemText,
  Menu,
  MenuItem,
  Tooltip,
  Typography,
  useMediaQuery,
} from '@mui/material';
import type { Theme } from '@mui/material/styles';
import { alpha, useTheme } from '@mui/material/styles';
import type { SxProps } from '@mui/system';
import { useEffect, useState } from 'react';
import { useLocation, useNavigate, useSearchParams } from 'react-router-dom';

const DRAWER_WIDTH = 280;
const COLLAPSED_WIDTH = 72;

export interface NavItem {
  title: string;
  path: string;
  icon: React.ReactNode;
  section?: string;
  children?: { title: string; path: string }[];
}

export interface SidebarProps {
  open: boolean;
  onClose: () => void;
  collapsed: boolean;
  onToggleCollapse: () => void;
  navItems: NavItem[];
  logo?: React.ReactNode;
  logoText?: string;
  user?: {
    name: string;
    role: string;
    avatar?: string;
  };
  onLogout?: () => void;
}

function activeAccentSx(isActive: boolean): SxProps<Theme> {
  if (!isActive) return {};
  return {
    '&::before': {
      content: '""',
      position: 'absolute',
      left: 4,
      top: '22%',
      bottom: '22%',
      width: 3,
      borderRadius: '0 2px 2px 0',
      bgcolor: 'primary.main',
    },
  };
}

function navItemSx(theme: Theme, isActive: boolean, collapsedDesktop: boolean): SxProps<Theme> {
  const primary = theme.palette.primary.main;
  return {
    position: 'relative',
    borderRadius: 2,
    mb: 0.5,
    justifyContent: collapsedDesktop ? 'center' : 'flex-start',
    px: collapsedDesktop ? 1.5 : 2,
    bgcolor: isActive ? alpha(primary, 0.15) : 'transparent',
    ...activeAccentSx(isActive),
    '&:hover': {
      bgcolor: isActive ? alpha(primary, 0.2) : alpha(theme.palette.common.white, 0.08),
    },
  };
}

function childNavItemSx(theme: Theme, isActive: boolean): SxProps<Theme> {
  const primary = theme.palette.primary.main;
  return {
    position: 'relative',
    pl: 6,
    py: 1,
    borderRadius: 2,
    mb: 0.5,
    bgcolor: isActive ? alpha(primary, 0.15) : 'transparent',
    ...activeAccentSx(isActive),
    '&:hover': {
      bgcolor: isActive ? alpha(primary, 0.2) : alpha(theme.palette.common.white, 0.08),
    },
  };
}

export default function Sidebar({
  open,
  onClose,
  collapsed,
  onToggleCollapse,
  navItems,
  logo,
  logoText = 'Admin',
  user = { name: 'John Doe', role: 'Administrator' },
  onLogout,
}: SidebarProps) {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const location = useLocation();
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const completePath = `${location.pathname}${
    searchParams.size > 0 ? `?` : ''
  }${searchParams.toString()}`;
  const [expandedItem, setExpandedItem] = useState<string | null>(null);
  const [flyoutAnchor, setFlyoutAnchor] = useState<HTMLElement | null>(null);
  const [flyoutItem, setFlyoutItem] = useState<NavItem | null>(null);

  const collapsedDesktop = collapsed && !isMobile;

  useEffect(() => {
    const activeParent = navItems.find((item) => {
      if (!item.children?.length) return false;
      if (item.path === '/') return location.pathname === '/';
      return location.pathname === item.path || location.pathname.startsWith(`${item.path}/`);
    });

    setExpandedItem(activeParent?.title ?? null);
  }, [location.pathname, navItems]);

  const handleExpand = (title: string) => {
    setExpandedItem((prev) => (prev === title ? null : title));
  };

  const handleParentClick = (item: NavItem, event: React.MouseEvent<HTMLElement>) => {
    if (collapsedDesktop && item.children?.length) {
      setFlyoutAnchor(event.currentTarget);
      setFlyoutItem(item);
      return;
    }
    setExpandedItem(item.title);
    navigate(item.path);
    if (isMobile) onClose();
  };

  const handleNavigate = (path: string) => {
    navigate(path);
    setFlyoutAnchor(null);
    setFlyoutItem(null);
    if (isMobile) onClose();
  };

  const isActive = (path: string) => {
    if (path === '/')
      return (
        completePath === `${path}${searchParams.size > 0 ? `?` : ''}${searchParams.toString()}`
      );
    return completePath.startsWith(path);
  };

  const isExactPath = (path: string) => completePath === path;

  const isAnyChildActive = (item: NavItem) =>
    item.children?.some((child) => isExactPath(child.path)) ?? false;

  const userInitials = user.name
    .split(' ')
    .map((n) => n[0])
    .join('');

  const logoFallbackSx = {
    width: 40,
    height: 40,
    borderRadius: 2,
    background: `linear-gradient(135deg, ${theme.palette.primary.light} 0%, ${theme.palette.primary.main} 100%)`,
    display: 'flex',
    alignItems: 'center',
    justifyContent: 'center',
    fontWeight: 700,
    fontSize: '1.25rem',
  };

  const renderSectionHeader = (section: string | undefined, index: number) => {
    if (!section || collapsedDesktop) return null;
    const prevSection = index > 0 ? navItems[index - 1]?.section : undefined;
    if (section === prevSection) return null;
    return (
      <Box key={`section-${section}-${index}`} sx={{ px: 1, pt: index === 0 ? 0 : 1.5, pb: 0.5 }}>
        <Divider sx={{ borderColor: alpha(theme.palette.common.white, 0.1), mb: 1 }} />
        <Typography
          variant="caption"
          sx={{
            color: alpha(theme.palette.common.white, 0.5),
            fontWeight: 600,
            letterSpacing: '0.08em',
            textTransform: 'uppercase',
            px: 1,
          }}
        >
          {section}
        </Typography>
      </Box>
    );
  };

  const drawerContent = (
    <Box
      sx={{
        height: '100%',
        display: 'flex',
        flexDirection: 'column',
        bgcolor: 'secondary.main',
        color: 'white',
      }}
    >
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: collapsedDesktop ? 'center' : 'space-between',
          p: 2,
          minHeight: 64,
        }}
      >
        {(!collapsed || isMobile) && (
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5 }}>
            {logo ?? <Box sx={logoFallbackSx}>{logoText.charAt(0)}</Box>}
            <Typography variant="h6" fontWeight={700}>
              {logoText}
            </Typography>
          </Box>
        )}
        {collapsedDesktop && (logo ?? <Box sx={logoFallbackSx}>{logoText.charAt(0)}</Box>)}
        {!isMobile && (
          <IconButton
            onClick={onToggleCollapse}
            sx={{
              color: 'white',
              ml: collapsedDesktop ? 0 : 'auto',
              bgcolor: alpha(theme.palette.common.white, 0.1),
              '&:hover': { bgcolor: alpha(theme.palette.common.white, 0.15) },
            }}
            size="small"
          >
            {collapsed ? <ChevronRight /> : <ChevronLeft />}
          </IconButton>
        )}
      </Box>

      <Divider sx={{ borderColor: alpha(theme.palette.common.white, 0.1) }} />

      <Box sx={{ flex: 1, overflow: 'auto', py: 2 }}>
        <List component="nav" sx={{ px: collapsedDesktop ? 0.5 : 1 }}>
          {navItems.map((item, index) => (
            <Box key={item.title}>
              {renderSectionHeader(item.section, index)}
              {item.children ? (
                <>
                  <Tooltip title={collapsedDesktop ? item.title : ''} placement="right">
                    <ListItemButton
                      onClick={(event) => handleParentClick(item, event)}
                      sx={navItemSx(theme, false, collapsedDesktop)}
                    >
                      <ListItemIcon
                        sx={{
                          color: isAnyChildActive(item) ? 'primary.main' : 'inherit',
                          minWidth: collapsedDesktop ? 0 : 40,
                        }}
                      >
                        {item.icon}
                      </ListItemIcon>
                      {(!collapsed || isMobile) && (
                        <>
                          <ListItemText primary={item.title} />
                          <IconButton
                            size="small"
                            aria-label={
                              expandedItem === item.title
                                ? `Collapse ${item.title}`
                                : `Expand ${item.title}`
                            }
                            onClick={(event) => {
                              event.preventDefault();
                              event.stopPropagation();
                              handleExpand(item.title);
                            }}
                            sx={{ color: 'inherit' }}
                          >
                            {expandedItem === item.title ? <ExpandLess /> : <ExpandMore />}
                          </IconButton>
                        </>
                      )}
                    </ListItemButton>
                  </Tooltip>
                  {(!collapsed || isMobile) && (
                    <Collapse in={expandedItem === item.title} timeout="auto">
                      <List component="div" disablePadding>
                        {item.children.map((child) => (
                          <ListItemButton
                            key={child.path}
                            onClick={() => handleNavigate(child.path)}
                            sx={childNavItemSx(theme, isExactPath(child.path))}
                          >
                            <ListItemText
                              primary={child.title}
                              primaryTypographyProps={{
                                fontSize: '0.875rem',
                                color: 'inherit',
                              }}
                            />
                          </ListItemButton>
                        ))}
                      </List>
                    </Collapse>
                  )}
                </>
              ) : (
                <Tooltip title={collapsedDesktop ? item.title : ''} placement="right">
                  <ListItemButton
                    onClick={() => handleNavigate(item.path)}
                    sx={navItemSx(theme, isActive(item.path), collapsedDesktop)}
                  >
                    <ListItemIcon
                      sx={{
                        color: isActive(item.path) ? 'primary.main' : 'inherit',
                        minWidth: collapsedDesktop ? 0 : 40,
                      }}
                    >
                      {item.icon}
                    </ListItemIcon>
                    {(!collapsed || isMobile) && <ListItemText primary={item.title} />}
                  </ListItemButton>
                </Tooltip>
              )}
            </Box>
          ))}
        </List>
      </Box>

      <Menu
        anchorEl={flyoutAnchor}
        open={Boolean(flyoutAnchor && flyoutItem)}
        onClose={() => {
          setFlyoutAnchor(null);
          setFlyoutItem(null);
        }}
        anchorOrigin={{ vertical: 'top', horizontal: 'right' }}
        transformOrigin={{ vertical: 'top', horizontal: 'left' }}
        slotProps={{
          paper: { sx: { minWidth: 200, ml: 0.5 } },
        }}
      >
        {flyoutItem?.children?.map((child) => (
          <MenuItem
            key={child.path}
            selected={completePath === child.path}
            onClick={() => handleNavigate(child.path)}
          >
            {child.title}
          </MenuItem>
        ))}
      </Menu>

      <Divider sx={{ borderColor: alpha(theme.palette.common.white, 0.1) }} />

      <Box sx={{ p: 2 }}>
        {collapsedDesktop ? (
          <Box sx={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 1 }}>
            <Tooltip title={user.name} placement="right">
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
            </Tooltip>
            {onLogout && (
              <Tooltip title="Log out" placement="right">
                <IconButton
                  size="small"
                  sx={{ color: alpha(theme.palette.common.white, 0.6) }}
                  onClick={onLogout}
                  aria-label="Log out"
                >
                  <Logout fontSize="small" />
                </IconButton>
              </Tooltip>
            )}
          </Box>
        ) : (
          <Box
            sx={{
              display: 'flex',
              alignItems: 'center',
              gap: 1.5,
              p: 1.5,
              borderRadius: 2,
              bgcolor: alpha(theme.palette.common.white, 0.05),
            }}
          >
            <Avatar
              sx={{
                bgcolor: 'primary.main',
                width: 40,
                height: 40,
                fontSize: '0.875rem',
              }}
            >
              {userInitials}
            </Avatar>
            <Box sx={{ flex: 1, minWidth: 0 }}>
              <Typography variant="body2" fontWeight={600} noWrap>
                {user.name}
              </Typography>
              <Typography
                variant="caption"
                sx={{ color: alpha(theme.palette.common.white, 0.6) }}
                noWrap
              >
                {user.role}
              </Typography>
            </Box>
            <IconButton
              size="small"
              sx={{ color: alpha(theme.palette.common.white, 0.6) }}
              onClick={onLogout}
              aria-label="Log out"
            >
              <Logout fontSize="small" />
            </IconButton>
          </Box>
        )}
      </Box>
    </Box>
  );

  return (
    <>
      {isMobile && (
        <Drawer
          variant="temporary"
          open={open}
          onClose={onClose}
          ModalProps={{ keepMounted: true }}
          sx={{
            display: { xs: 'block', md: 'none' },
            '& .MuiDrawer-paper': {
              width: DRAWER_WIDTH,
              boxSizing: 'border-box',
            },
          }}
        >
          {drawerContent}
        </Drawer>
      )}

      {!isMobile && (
        <Drawer
          variant="permanent"
          sx={{
            display: { xs: 'none', md: 'block' },
            '& .MuiDrawer-paper': {
              width: collapsed ? COLLAPSED_WIDTH : DRAWER_WIDTH,
              boxSizing: 'border-box',
              transition: theme.transitions.create('width', {
                easing: theme.transitions.easing.sharp,
                duration: theme.transitions.duration.enteringScreen,
              }),
              overflowX: 'hidden',
            },
          }}
        >
          {drawerContent}
        </Drawer>
      )}
    </>
  );
}
