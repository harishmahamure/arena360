import type { NavItem } from '@gaming-cafe/ui';
import {
  CardMembership,
  Dashboard,
  Devices,
  FileUpload,
  Inventory,
  Link,
  People,
  PlayCircle,
  Receipt,
  Sell,
  SportsEsports,
} from '@mui/icons-material';

export const adminNavItems: NavItem[] = [
  { title: 'Dashboard', path: '/', icon: <Dashboard /> },
  {
    title: 'Sessions',
    path: '/sessions',
    icon: <PlayCircle />,
    children: [
      { title: 'All Sessions', path: '/sessions' },
      { title: 'Start New Session', path: '/sessions/new' },
      { title: 'Active Sessions', path: '/sessions?active=true' },
      { title: 'Completed Sessions', path: '/sessions?active=false' },
    ],
  },
  {
    title: 'Players',
    path: '/players',
    icon: <People />,
    children: [
      { title: 'All Players', path: '/players' },
      { title: 'Add Player', path: '/players/new' },
      { title: 'Admins', path: '/players?role=admin' },
      { title: 'Inactive', path: '/players?active=false' },
    ],
  },
  {
    title: 'Plan Assignments',
    path: '/plan-transactions',
    icon: <Receipt />,
    children: [
      { title: 'All Transactions', path: '/plan-transactions' },
      { title: 'New Transaction', path: '/plan-transactions/new' },
      { title: 'Pending', path: '/plan-transactions?status=pending' },
      { title: 'Completed', path: '/plan-transactions?status=completed' },
      { title: 'Failed', path: '/plan-transactions?status=failed' },
    ],
  },

  {
    title: 'Products',
    path: '/products',
    icon: <Inventory />,
    children: [
      { title: 'All Products', path: '/products' },
      { title: 'Add Product', path: '/products/new' },
      { title: 'Expiring Soon', path: '/products?stockExpiringSoon=true' },
      { title: 'Deleted Products', path: '/products?deleted=true' },
    ],
  },
  // {
  //   title: "Purchase Orders",
  //   path: "/purchase-orders",
  //   icon: <ShoppingCart />,
  //   children: [
  //     { title: "All Purchase Orders", path: "/purchase-orders" },
  //     { title: "Add Purchase Order", path: "/purchase-orders/new" },
  //     { title: "Pending", path: "/purchase-orders?status=pending" },
  //     { title: "Completed", path: "/purchase-orders?status=completed" },
  //   ],
  // },
  {
    title: 'Sell Items',
    path: '/product-transactions',
    icon: <Sell />,
    children: [
      { title: 'All Sold Items', path: '/product-transactions' },
      { title: 'Sell New Items', path: '/product-transactions/new' },
      {
        title: 'Pending Transactions',
        path: '/product-transactions?status=pending',
      },
      {
        title: 'Completed Transactions',
        path: '/product-transactions?status=completed',
      },
    ],
  },

  {
    title: 'Devices',
    path: '/devices',
    icon: <Devices />,
    children: [
      { title: 'All Devices', path: '/devices' },
      { title: 'Add Device', path: '/devices/new' },
      { title: 'Operational', path: '/devices?status=operational' },
      { title: 'Maintenance', path: '/devices?status=under_maintenance' },
      { title: 'Out of Order', path: '/devices?status=out_of_service' },
    ],
  },
  {
    title: 'Plans',
    path: '/plans',
    icon: <CardMembership />,
    children: [
      { title: 'All Plans', path: '/plans' },
      { title: 'Add Plan', path: '/plans/new' },
      { title: 'Time Based', path: '/plans?planType=time_based' },
      { title: 'Session Based', path: '/plans?planType=session_based' },
      { title: 'Unlimited Daily', path: '/plans?planType=unlimited_daily' },
      {
        title: 'Monthly Subscription',
        path: '/plans?planType=monthly_subscription',
      },
      { title: 'Inactive Plans', path: '/plans?isActive=false' },
    ],
  },
  {
    title: 'Games',
    path: '/games',
    icon: <SportsEsports />,
    children: [
      { title: 'All Games', path: '/games' },
      { title: 'Add Game', path: '/games/new' },
      { title: 'Multiplayer', path: '/games?multiplayer=true' },
      { title: 'Inactive Games', path: '/games?deleted=true' },
    ],
  },
  {
    title: 'Device Games',
    path: '/device-games',
    icon: <Link />,
    children: [
      { title: 'All Assignments', path: '/device-games' },
      { title: 'Assign Game', path: '/device-games/new' },
      { title: 'Inactive', path: '/device-games?active=false' },
    ],
  },

  {
    title: 'Upload Videos & Images',
    path: '/media/upload',
    icon: <FileUpload />,
  },
  // {
  //   title: "Units",
  //   path: "/units",
  //   icon: <Straighten />,
  //   children: [
  //     { title: "All Units", path: "/units" },
  //     { title: "Add Unit", path: "/units/new" },
  //     { title: "Inactive Units", path: "/units?active=false" },
  //   ],
  // },
];
