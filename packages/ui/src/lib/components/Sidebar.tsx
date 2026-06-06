import {
  BarChart,
  ChevronLeft,
  ChevronRight,
  Dashboard,
  ExpandLess,
  ExpandMore,
  Group,
  Inventory,
  LocalOffer,
  Logout,
  People,
  Receipt,
  Settings,
  ShoppingCart,
  Storefront,
} from '@mui/icons-material';
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
  Tooltip,
  Typography,
  useMediaQuery,
  useTheme,
} from '@mui/material';
import { useState } from 'react';
import { Link, useLocation, useSearchParams } from 'react-router-dom';

const DRAWER_WIDTH = 280;
const COLLAPSED_WIDTH = 72;

export interface NavItem {
  title: string;
  path: string;
  icon: React.ReactNode;
  children?: { title: string; path: string }[];
}

const defaultNavItems: NavItem[] = [
  { title: 'Dashboard', path: '/', icon: <Dashboard /> },
  {
    title: 'Products',
    path: '/products',
    icon: <Inventory />,
    children: [
      { title: 'All Products', path: '/products' },
      { title: 'Add Product', path: '/products/add' },
      { title: 'Categories', path: '/products/categories' },
    ],
  },
  {
    title: 'Orders',
    path: '/orders',
    icon: <ShoppingCart />,
    children: [
      { title: 'All Orders', path: '/orders' },
      { title: 'Pending', path: '/orders/pending' },
      { title: 'Completed', path: '/orders/completed' },
    ],
  },
  { title: 'Customers', path: '/customers', icon: <People /> },
  { title: 'Analytics', path: '/analytics', icon: <BarChart /> },
  { title: 'Store', path: '/store', icon: <Storefront /> },
  { title: 'Transactions', path: '/transactions', icon: <Receipt /> },
  { title: 'Promotions', path: '/promotions', icon: <LocalOffer /> },
  { title: 'Team', path: '/team', icon: <Group /> },
  { title: 'Settings', path: '/settings', icon: <Settings /> },
];

export interface SidebarProps {
  open: boolean;
  onClose: () => void;
  collapsed: boolean;
  onToggleCollapse: () => void;
  navItems?: NavItem[];
  logo?: React.ReactNode;
  logoText?: string;
  user?: {
    name: string;
    role: string;
    avatar?: string;
  };
}

export default function Sidebar({
  open,
  onClose,
  collapsed,
  onToggleCollapse,
  navItems = defaultNavItems,
  logo,
  logoText = 'Admin',
  user = { name: 'John Doe', role: 'Administrator' },
}: SidebarProps) {
  const theme = useTheme();
  const isMobile = useMediaQuery(theme.breakpoints.down('md'));
  const location = useLocation();
  const [searchParams] = useSearchParams();
  const completePath = `${location.pathname}${
    searchParams.size > 0 ? `?` : ''
  }${searchParams.toString()}`;
  const [expandedItems, setExpandedItems] = useState<string[]>([]);

  const handleExpand = (title: string) => {
    setExpandedItems((prev) =>
      prev.includes(title) ? prev.filter((item) => item !== title) : [...prev, title],
    );
  };

  const isActive = (path: string) => {
    if (path === '/')
      return (
        completePath === `${path}${searchParams.size > 0 ? `?` : ''}${searchParams.toString()}`
      );
    return completePath.startsWith(path);
  };

  const userInitials = user.name
    .split(' ')
    .map((n) => n[0])
    .join('');

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
      {/* Logo Section */}
      <Box
        sx={{
          display: 'flex',
          alignItems: 'center',
          justifyContent: collapsed && !isMobile ? 'center' : 'space-between',
          p: 2,
          minHeight: 64,
        }}
      >
        {(!collapsed || isMobile) && (
          <Box sx={{ display: 'flex', alignItems: 'center', gap: 1.5 }}>
            {logo ?? (
              <Box
                sx={{
                  width: 40,
                  height: 40,
                  borderRadius: 2,
                  background: 'linear-gradient(135deg, #FF6900 0%, #CC5400 100%)',
                  display: 'flex',
                  alignItems: 'center',
                  justifyContent: 'center',
                  fontWeight: 700,
                  fontSize: '1.25rem',
                }}
              >
                {logoText.charAt(0)}
              </Box>
            )}
            <Typography variant="h6" fontWeight={700}>
              {logoText}
            </Typography>
          </Box>
        )}
        {collapsed && !isMobile && (
          logo ?? (
            <Box
              sx={{
                width: 40,
                height: 40,
                borderRadius: 2,
                background: 'linear-gradient(135deg, #FF6900 0%, #CC5400 100%)',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                fontWeight: 700,
                fontSize: '1.25rem',
              }}
            >
              {logoText.charAt(0)}
            </Box>
          )
        )}
        {!isMobile && (
          <IconButton
            onClick={onToggleCollapse}
            sx={{
              color: 'white',
              ml: collapsed ? 0 : 'auto',
              bgcolor: 'rgba(255,255,255,0.1)',
              '&:hover': { bgcolor: 'rgba(255,255,255,0.15)' },
            }}
            size="small"
          >
            {collapsed ? <ChevronRight /> : <ChevronLeft />}
          </IconButton>
        )}
      </Box>

      <Divider sx={{ borderColor: 'rgba(255,255,255,0.1)' }} />

      {/* Navigation */}
      <Box sx={{ flex: 1, overflow: 'auto', py: 2 }}>
        <List component="nav" sx={{ px: collapsed && !isMobile ? 0.5 : 1 }}>
          {navItems.map((item) => (
            <Box key={item.title}>
              {item.children ? (
                <>
                  <Tooltip title={collapsed && !isMobile ? item.title : ''} placement="right">
                    <ListItemButton
                      onClick={() => handleExpand(item.title)}
                      sx={{
                        borderRadius: 2,
                        mb: 0.5,
                        justifyContent: collapsed && !isMobile ? 'center' : 'flex-start',
                        px: collapsed && !isMobile ? 1.5 : 2,
                        bgcolor: isActive(item.path) ? 'rgba(255,105,0,0.2)' : 'transparent',
                        '&:hover': {
                          bgcolor: isActive(item.path)
                            ? 'rgba(255,105,0,0.25)'
                            : 'rgba(255,255,255,0.08)',
                        },
                      }}
                    >
                      <ListItemIcon
                        sx={{
                          color: isActive(item.path) ? 'primary.main' : 'inherit',
                          minWidth: collapsed && !isMobile ? 0 : 40,
                        }}
                      >
                        {item.icon}
                      </ListItemIcon>
                      {(!collapsed || isMobile) && (
                        <>
                          <ListItemText primary={item.title} />
                          {expandedItems.includes(item.title) ? <ExpandLess /> : <ExpandMore />}
                        </>
                      )}
                    </ListItemButton>
                  </Tooltip>
                  {(!collapsed || isMobile) && (
                    <Collapse in={expandedItems.includes(item.title)} timeout="auto">
                      <List component="div" disablePadding>
                        {item.children.map((child) => (
                          <ListItemButton
                            key={child.path}
                            component={Link}
                            to={child.path}
                            onClick={isMobile ? onClose : undefined}
                            sx={{
                              pl: 6,
                              py: 1,
                              borderRadius: 2,
                              mb: 0.5,
                              bgcolor:
                                completePath === child.path ? 'rgba(255,105,0,0.2)' : 'transparent',
                              '&:hover': {
                                bgcolor:
                                  completePath === child.path
                                    ? 'rgba(255,105,0,0.25)'
                                    : 'rgba(255,255,255,0.08)',
                              },
                            }}
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
                <Tooltip title={collapsed && !isMobile ? item.title : ''} placement="right">
                  <ListItemButton
                    component={Link}
                    to={item.path}
                    onClick={isMobile ? onClose : undefined}
                    sx={{
                      borderRadius: 2,
                      mb: 0.5,
                      justifyContent: collapsed && !isMobile ? 'center' : 'flex-start',
                      px: collapsed && !isMobile ? 1.5 : 2,
                      bgcolor: isActive(item.path) ? 'rgba(255,105,0,0.2)' : 'transparent',
                      '&:hover': {
                        bgcolor: isActive(item.path)
                          ? 'rgba(255,105,0,0.25)'
                          : 'rgba(255,255,255,0.08)',
                      },
                    }}
                  >
                    <ListItemIcon
                      sx={{
                        color: isActive(item.path) ? 'primary.main' : 'inherit',
                        minWidth: collapsed && !isMobile ? 0 : 40,
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

      <Divider sx={{ borderColor: 'rgba(255,255,255,0.1)' }} />

      {/* User Section */}
      <Box sx={{ p: 2 }}>
        {collapsed && !isMobile ? (
          <Box sx={{ display: 'flex', justifyContent: 'center' }}>
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
          </Box>
        ) : (
          <Box
            sx={{
              display: 'flex',
              alignItems: 'center',
              gap: 1.5,
              p: 1.5,
              borderRadius: 2,
              bgcolor: 'rgba(255,255,255,0.05)',
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
              <Typography variant="caption" sx={{ color: 'rgba(255,255,255,0.6)' }} noWrap>
                {user.role}
              </Typography>
            </Box>
            <IconButton size="small" sx={{ color: 'rgba(255,255,255,0.6)' }}>
              <Logout fontSize="small" />
            </IconButton>
          </Box>
        )}
      </Box>
    </Box>
  );

  return (
    <>
      {/* Mobile Drawer */}
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

      {/* Desktop Drawer */}
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
