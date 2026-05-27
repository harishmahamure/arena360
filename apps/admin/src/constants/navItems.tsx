import { Permission } from '@gaming-cafe/contracts';
import {
  CardMembership,
  Dashboard,
  Devices,
  FileUpload,
  Inventory,
  Link,
  MoneyOff,
  People,
  PlayCircle,
  PointOfSale,
  Receipt,
  Sell,
  Settings,
  SportsEsports,
  Store,
  WorkHistory,
} from '@mui/icons-material';
import type { AdminNavItem } from '../utils/filterNavItems';

export const adminNavItems: AdminNavItem[] = [
  { title: 'Dashboard', path: '/', icon: <Dashboard />, requiredPermission: Permission.StatsRead },
  {
    title: 'Sessions',
    path: '/sessions',
    icon: <PlayCircle />,
    requiredPermission: Permission.SessionsRead,
    children: [
      { title: 'All Sessions', path: '/sessions' },
      {
        title: 'Start New Session',
        path: '/sessions/new',
        requiredPermission: Permission.SessionsWrite,
      },
      { title: 'Active Sessions', path: '/sessions?active=true' },
      { title: 'Completed Sessions', path: '/sessions?active=false' },
    ],
  },
  {
    title: 'Players',
    path: '/players',
    icon: <People />,
    requiredPermission: Permission.PlayersRead,
    children: [
      { title: 'All Players', path: '/players' },
      {
        title: 'Add Player',
        path: '/players/new',
        requiredPermission: Permission.PlayersWrite,
      },
      { title: 'Staff', path: '/players?role=staff' },
      { title: 'Admins', path: '/players?role=admin' },
      { title: 'Inactive', path: '/players?active=false' },
    ],
  },
  {
    title: 'Plan Assignments',
    path: '/plan-transactions',
    icon: <Receipt />,
    requiredPermission: Permission.PlayerPlansRead,
    children: [
      { title: 'All Transactions', path: '/plan-transactions' },
      {
        title: 'New Transaction',
        path: '/plan-transactions/new',
        requiredPermission: Permission.PlayerPlansWrite,
      },
      { title: 'Pending', path: '/plan-transactions?status=pending' },
      { title: 'Completed', path: '/plan-transactions?status=completed' },
      { title: 'Failed', path: '/plan-transactions?status=failed' },
    ],
  },
  {
    title: 'Products',
    path: '/products',
    icon: <Inventory />,
    requiredPermission: Permission.ProductsRead,
    children: [
      { title: 'All Products', path: '/products' },
      {
        title: 'Add Product',
        path: '/products/new',
        requiredPermission: Permission.ProductsWrite,
      },
      { title: 'Expiring Soon', path: '/products?stockExpiringSoon=true' },
      { title: 'Inactive Products', path: '/products?deleted=true' },
    ],
  },
  {
    title: 'Sell Items',
    path: '/product-transactions',
    icon: <Sell />,
    requiredPermission: Permission.TransactionsRead,
    children: [
      { title: 'All Sold Items', path: '/product-transactions' },
      {
        title: 'Sell New Items',
        path: '/product-transactions/new',
        requiredPermission: Permission.TransactionsWrite,
      },
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
    requiredPermission: Permission.DevicesRead,
    children: [
      { title: 'All Devices', path: '/devices' },
      {
        title: 'Add Device',
        path: '/devices/new',
        requiredPermission: Permission.DevicesWrite,
      },
      { title: 'Operational', path: '/devices?status=operational' },
      { title: 'Maintenance', path: '/devices?status=under_maintenance' },
      { title: 'Out of Order', path: '/devices?status=out_of_service' },
    ],
  },
  {
    title: 'Plans',
    path: '/plans',
    icon: <CardMembership />,
    requiredPermission: Permission.PlansRead,
    children: [
      { title: 'All Plans', path: '/plans' },
      {
        title: 'Add Plan',
        path: '/plans/new',
        requiredPermission: Permission.PlansWrite,
      },
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
    requiredPermission: Permission.GamesRead,
    children: [
      { title: 'All Games', path: '/games' },
      {
        title: 'Add Game',
        path: '/games/new',
        requiredPermission: Permission.GamesWrite,
      },
      { title: 'Multiplayer', path: '/games?multiplayer=true' },
      { title: 'Inactive Games', path: '/games?deleted=true' },
    ],
  },
  {
    title: 'Device Games',
    path: '/device-games',
    icon: <Link />,
    requiredPermission: Permission.DeviceGamesRead,
    children: [
      { title: 'All Assignments', path: '/device-games' },
      {
        title: 'Assign Game',
        path: '/device-games/new',
        requiredPermission: Permission.DeviceGamesWrite,
      },
      { title: 'Inactive', path: '/device-games?active=false' },
    ],
  },
  {
    title: 'Upload Videos & Images',
    path: '/media/upload',
    icon: <FileUpload />,
    requiredPermission: Permission.FilesWrite,
  },
  {
    title: 'Shifts',
    path: '/shifts',
    icon: <WorkHistory />,
    requiredPermission: Permission.ShiftsRead,
  },
  {
    title: 'Cash Registers',
    path: '/cash-registers',
    icon: <PointOfSale />,
    requiredPermission: Permission.CashRegistersRead,
  },
  {
    title: 'Cash Deposits',
    path: '/cash-deposits',
    icon: <PointOfSale />,
    requiredPermission: Permission.CashDepositsRead,
    children: [
      { title: 'All Deposits', path: '/cash-deposits' },
      { title: 'Pending Approval', path: '/cash-deposits?status=pending' },
      { title: 'Approved', path: '/cash-deposits?status=approved' },
    ],
  },
  {
    title: 'Expenses',
    path: '/expenses',
    icon: <MoneyOff />,
    requiredPermission: Permission.ExpensesRead,
    children: [
      { title: 'All Expenses', path: '/expenses' },
      {
        title: 'Add Expense',
        path: '/expenses/new',
        requiredPermission: Permission.ExpensesWrite,
      },
      { title: 'Pending Approval', path: '/expenses?status=pending' },
      { title: 'Approved', path: '/expenses?status=approved' },
    ],
  },
  {
    title: 'Vendors',
    path: '/vendors',
    icon: <Store />,
    requiredPermission: Permission.VendorsRead,
  },
  {
    title: 'Settings',
    path: '/settings',
    icon: <Settings />,
    requiredPermission: Permission.ConfigRead,
  },
];
