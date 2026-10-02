import { Permission } from '@gaming-cafe/contracts';
import { TOAST_CONTAINER_PROPS } from '@gaming-cafe/utils';
import { CssBaseline } from '@mui/material';
import { ToastContainer } from 'react-toastify';
import { AppearanceProvider } from './theme/AppearanceProvider';
import 'react-toastify/dist/ReactToastify.css';
import { isApiError, local, toastUtils } from '@gaming-cafe/utils';
import { LinearProgress } from '@mui/material';
import { AdapterDateFns } from '@mui/x-date-pickers/AdapterDateFns';
import { LocalizationProvider } from '@mui/x-date-pickers/LocalizationProvider';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { lazy, Suspense, useEffect, useReducer, useRef } from 'react';
import { Route, Routes, useNavigate } from 'react-router-dom';
import ErrorBoundary from './components/ErrorBoundary';
import RequirePermission from './components/RequirePermission';
import SessionVerifier from './components/SessionVerifier';
import AuthLayout from './layouts/AuthLayout';
import DashboardLayout from './layouts/DashboardLayout';
import {
  bootstrapAuthFromToken,
  clearAdminSession,
  registerAdminAuthSession,
  watchSessionExpiry,
} from './lib/authSession';
import { RealtimeProvider } from './lib/realtime';

import { StoreContext } from './store';
import { PERSIST_KEY } from './store/persistance';
import { rootInitialState, rootReducer } from './store/rootReducer';

const LoginPage = lazy(() => import('./pages/auth/LoginPage'));
const ShiftSetupPage = lazy(() => import('./pages/auth/ShiftSetupPage'));
const ActivityLogPage = lazy(() => import('./pages/dashboard/ActivityLogPage'));
const CashDepositsPage = lazy(() => import('./pages/dashboard/cash-deposits/CashDepositsPage'));
const CashRegisterDetailPage = lazy(
  () => import('./pages/dashboard/cash-registers/CashRegisterDetailPage'),
);
const CashRegistersPage = lazy(() => import('./pages/dashboard/cash-registers/CashRegistersPage'));
const CreditPage = lazy(() => import('./pages/dashboard/credit/CreditPage'));
const CreditSettlementDetailPage = lazy(
  () => import('./pages/dashboard/credit/CreditSettlementDetailPage'),
);
const CreditSettlementsPage = lazy(() => import('./pages/dashboard/credit/CreditSettlementsPage'));
const DashboardPage = lazy(() => import('./pages/dashboard/DashboardPage'));
const DeviceDetailPage = lazy(() => import('./pages/dashboard/devices/DeviceDetailPage'));
const DeviceNewPage = lazy(() => import('./pages/dashboard/devices/DeviceNewPage'));
const DevicesPage = lazy(() => import('./pages/dashboard/devices/DevicesPage'));
const ExpenseDetailPage = lazy(() => import('./pages/dashboard/expenses/ExpenseDetailPage'));
const ExpenseNewPage = lazy(() => import('./pages/dashboard/expenses/ExpenseNewPage'));
const ExpensesPage = lazy(() => import('./pages/dashboard/expenses/ExpensesPage'));
const FinanceDepositsPage = lazy(() => import('./pages/dashboard/finance/FinanceDepositsPage'));
const FinanceReconciliationPage = lazy(
  () => import('./pages/dashboard/finance/FinanceReconciliationPage'),
);
const FinanceVariancePage = lazy(() => import('./pages/dashboard/finance/FinanceVariancePage'));
const GameDetailPage = lazy(() => import('./pages/dashboard/games/GameDetailPage'));
const GameNewPage = lazy(() => import('./pages/dashboard/games/GameNewPage'));
const GamesPage = lazy(() => import('./pages/dashboard/games/GamesPage'));
const InventoryLocationsPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryLocationsPage'),
);
const InventoryMovementsPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryMovementsPage'),
);
const InventoryOverviewPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryOverviewPage'),
);
const InventoryReceiptReportPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryReceiptReportPage'),
);
const InventoryReorderPage = lazy(() => import('./pages/dashboard/inventory/InventoryReorderPage'));
const InventoryStockPage = lazy(() => import('./pages/dashboard/inventory/InventoryStockPage'));
const InventoryTransferDetailPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryTransferDetailPage'),
);
const InventoryTransferNewPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryTransferNewPage'),
);
const InventoryTransfersPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryTransfersPage'),
);
const InventoryWarehousePage = lazy(
  () => import('./pages/dashboard/inventory/InventoryWarehousePage'),
);
const InventoryWasteNewPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryWasteNewPage'),
);
const InventoryWastePage = lazy(() => import('./pages/dashboard/inventory/InventoryWastePage'));
const InventoryWasteReportPage = lazy(
  () => import('./pages/dashboard/inventory/InventoryWasteReportPage'),
);
const PurchaseOrderDetailPage = lazy(
  () => import('./pages/dashboard/inventory/PurchaseOrderDetailPage'),
);
const PurchaseOrderNewPage = lazy(() => import('./pages/dashboard/inventory/PurchaseOrderNewPage'));
const PurchaseOrdersPage = lazy(() => import('./pages/dashboard/inventory/PurchaseOrdersPage'));
const KitchenPage = lazy(() => import('./pages/dashboard/kitchen/KitchenPage'));
const FinanceReportPage = lazy(() => import('./pages/dashboard/finance/FinanceReportPage'));
const KioskOrdersPage = lazy(() => import('./pages/dashboard/kiosk-orders/KioskOrdersPage'));
const PlanTransactionDetailPage = lazy(
  () => import('./pages/dashboard/plan-transactions/PlanTransactionDetailPage'),
);
const PlanTransactionNewPage = lazy(
  () => import('./pages/dashboard/plan-transactions/PlanTransactionNewPage'),
);
const PlanTransactionsPage = lazy(
  () => import('./pages/dashboard/plan-transactions/PlanTransactionsPage'),
);
const PlanDetailPage = lazy(() => import('./pages/dashboard/plans/PlanDetailPage'));
const PlanNewPage = lazy(() => import('./pages/dashboard/plans/PlanNewPage'));
const PlansPage = lazy(() => import('./pages/dashboard/plans/PlansPage'));
const PlayerDetailPage = lazy(() => import('./pages/dashboard/players/PlayerDetailPage'));
const PlayerNewPage = lazy(() => import('./pages/dashboard/players/PlayerNewPage'));
const PlayersPage = lazy(() => import('./pages/dashboard/players/PlayersPage'));
const ProductTransactionDetailPage = lazy(
  () => import('./pages/dashboard/product-transactions/ProductTransactionDetailPage'),
);
const ProductTransactionNewPage = lazy(
  () => import('./pages/dashboard/product-transactions/ProductTransactionNewPage'),
);
const ProductTransactionsPage = lazy(
  () => import('./pages/dashboard/product-transactions/ProductTransactionsPage'),
);
const ProductDetailPage = lazy(() => import('./pages/dashboard/products/ProductDetailPage'));
const ProductNewPage = lazy(() => import('./pages/dashboard/products/ProductNewPage'));
const ProductsPage = lazy(() => import('./pages/dashboard/products/ProductsPage'));
const SessionDetailPage = lazy(() => import('./pages/dashboard/sessions/SessionDetailPage'));
const SessionNewPage = lazy(() => import('./pages/dashboard/sessions/SessionNewPage'));
const SessionsPage = lazy(() => import('./pages/dashboard/sessions/SessionsPage'));
const SettingsPage = lazy(() => import('./pages/dashboard/settings/SettingsPage'));
const ShiftDetailPage = lazy(() => import('./pages/dashboard/shifts/ShiftDetailPage'));
const ShiftsPage = lazy(() => import('./pages/dashboard/shifts/ShiftsPage'));
const StationsFloorPage = lazy(() => import('./pages/dashboard/stations/StationsFloorPage'));
const VendorDetailPage = lazy(() => import('./pages/dashboard/vendors/VendorDetailPage'));
const VendorNewPage = lazy(() => import('./pages/dashboard/vendors/VendorNewPage'));
const VendorsPage = lazy(() => import('./pages/dashboard/vendors/VendorsPage'));
const NotFoundPage = lazy(() => import('./pages/NotFoundPage'));

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      retry: (count, error) => count < 1 && (!isApiError(error) || error.statusCode >= 500),
      staleTime: 15_000,
    },
  },
});

function App() {
  const [state, dispatch] = useReducer(rootReducer, rootInitialState);
  const navigate = useNavigate();
  const navigation = useRef(navigate);
  useEffect(() => {
    navigation.current = navigate;
  }, [navigate]);

  useEffect(() => {
    const reset = () => {
      void queryClient.cancelQueries();
      queryClient.clear();
      dispatch({ type: 'Reset' });
    };
    const unregister = registerAdminAuthSession({
      onSessionExpired: () => {
        toastUtils.warning('Session expired — please sign in again');
        navigation.current('/login', { replace: true });
      },
    });
    let stopExpiry = () => {};
    const sync = () => {
      stopExpiry();
      reset();
      bootstrapAuthFromToken(dispatch);
      stopExpiry = watchSessionExpiry(() => {
        clearAdminSession();
        toastUtils.warning('Session expired — please sign in again');
        navigation.current('/login', { replace: true });
      });
    };
    const refresh = () => {
      void queryClient.invalidateQueries();
    };
    const storage = (event: StorageEvent) => {
      if (event.key === 'arena:data-revision') refresh();
      if (event.key === 'accessToken' || event.key === null) sync();
    };
    sync();
    window.addEventListener('arena:session-change', sync);
    window.addEventListener('storage', storage);
    window.addEventListener('arena:data-change', refresh);
    return () => {
      unregister();
      stopExpiry();
      window.removeEventListener('arena:session-change', sync);
      window.removeEventListener('storage', storage);
      window.removeEventListener('arena:data-change', refresh);
    };
  }, []);

  useEffect(() => {
    if (state.auth.id && local.get('accessToken')) local.set(PERSIST_KEY, JSON.stringify(state));
  }, [state]);

  return (
    <ErrorBoundary>
      <AppearanceProvider accountId={state.auth.id}>
        <CssBaseline />
        <ToastContainer {...TOAST_CONTAINER_PROPS} theme="colored" />
        <LocalizationProvider dateAdapter={AdapterDateFns}>
          <StoreContext value={{ dispatch, state }}>
            <QueryClientProvider client={queryClient}>
              <RealtimeProvider>
                <SessionVerifier />
                <Suspense fallback={<LinearProgress aria-label="Loading page" />}>
                  <Routes>
                    <Route element={<AuthLayout />}>
                      <Route path="/login" element={<LoginPage />} />
                      <Route path="/shift/setup" element={<ShiftSetupPage />} />
                    </Route>
                    <Route element={<DashboardLayout />}>
                      <Route path="/" element={<DashboardPage />} />
                      <Route element={<RequirePermission permission={Permission.PlayersRead} />}>
                        <Route path="/players" element={<PlayersPage />} />
                        <Route path="/players/:id" element={<PlayerDetailPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.PlayersWrite} />}>
                        <Route path="/players/new" element={<PlayerNewPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.DevicesRead} />}>
                        <Route path="/devices" element={<DevicesPage />} />
                        <Route path="/devices/:id" element={<DeviceDetailPage />} />
                        <Route path="/stations" element={<StationsFloorPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.DevicesWrite} />}>
                        <Route path="/devices/new" element={<DeviceNewPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.PlansRead} />}>
                        <Route path="/plans" element={<PlansPage />} />
                        <Route path="/plans/:id" element={<PlanDetailPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.PlansWrite} />}>
                        <Route path="/plans/new" element={<PlanNewPage />} />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.PlayerPlansRead} />}
                      >
                        <Route path="/plan-transactions" element={<PlanTransactionsPage />} />
                        <Route
                          path="/plan-transactions/:id"
                          element={<PlanTransactionDetailPage />}
                        />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.PlayerPlansWrite} />}
                      >
                        <Route path="/plan-transactions/new" element={<PlanTransactionNewPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.SessionsRead} />}>
                        <Route path="/sessions" element={<SessionsPage />} />
                        <Route path="/sessions/:id" element={<SessionDetailPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.SessionsWrite} />}>
                        <Route path="/sessions/new" element={<SessionNewPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.ProductsRead} />}>
                        <Route path="/products" element={<ProductsPage />} />
                        <Route path="/products/:id" element={<ProductDetailPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.ProductsWrite} />}>
                        <Route path="/products/new" element={<ProductNewPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.GamesRead} />}>
                        <Route path="/games" element={<GamesPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.GamesWrite} />}>
                        <Route path="/games/new" element={<GameNewPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.GamesRead} />}>
                        <Route path="/games/:id" element={<GameDetailPage />} />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.TransactionsRead} />}
                      >
                        <Route path="/kitchen" element={<KitchenPage />} />
                        <Route path="/kiosk-orders" element={<KioskOrdersPage />} />
                        <Route path="/product-transactions" element={<ProductTransactionsPage />} />
                        <Route
                          path="/product-transactions/:id"
                          element={<ProductTransactionDetailPage />}
                        />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.TransactionsWrite} />}
                      >
                        <Route
                          path="/product-transactions/new"
                          element={<ProductTransactionNewPage />}
                        />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.ShiftsRead} />}>
                        <Route path="/shifts" element={<ShiftsPage />} />
                        <Route path="/shifts/:id" element={<ShiftDetailPage />} />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.CashRegistersRead} />}
                      >
                        <Route path="/cash-registers" element={<CashRegistersPage />} />
                        <Route path="/cash-registers/:id" element={<CashRegisterDetailPage />} />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.CashDepositsRead} />}
                      >
                        <Route path="/cash-deposits" element={<CashDepositsPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.CreditRead} />}>
                        <Route path="/credit" element={<CreditPage />} />
                        <Route path="/credit/settlements" element={<CreditSettlementsPage />} />
                        <Route
                          path="/credit/settlements/:id"
                          element={<CreditSettlementDetailPage />}
                        />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.ExpensesRead} />}>
                        <Route path="/expenses" element={<ExpensesPage />} />
                        <Route path="/expenses/:id" element={<ExpenseDetailPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.ExpensesWrite} />}>
                        <Route path="/expenses/new" element={<ExpenseNewPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.VendorsRead} />}>
                        <Route path="/vendors" element={<VendorsPage />} />
                        <Route path="/vendors/:id" element={<VendorDetailPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.VendorsWrite} />}>
                        <Route path="/vendors/new" element={<VendorNewPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.InventoryRead} />}>
                        <Route path="/inventory" element={<InventoryOverviewPage />} />
                        <Route path="/inventory/stock" element={<InventoryStockPage />} />
                        <Route path="/inventory/movements" element={<InventoryMovementsPage />} />
                        <Route path="/inventory/transfers" element={<InventoryTransfersPage />} />
                        <Route
                          path="/inventory/transfers/:id"
                          element={<InventoryTransferDetailPage />}
                        />
                        <Route
                          path="/inventory/transfers/new"
                          element={<InventoryTransferNewPage />}
                        />
                        <Route path="/inventory/waste" element={<InventoryWastePage />} />
                        <Route path="/inventory/waste/new" element={<InventoryWasteNewPage />} />
                        <Route
                          path="/inventory/waste/report"
                          element={<InventoryWasteReportPage />}
                        />
                        <Route
                          path="/inventory/receipts/report"
                          element={<InventoryReceiptReportPage />}
                        />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.ProcurementRead} />}
                      >
                        <Route path="/inventory/purchase-orders" element={<PurchaseOrdersPage />} />
                        <Route
                          path="/inventory/purchase-orders/:id"
                          element={<PurchaseOrderDetailPage />}
                        />
                        <Route path="/inventory/reorder" element={<InventoryReorderPage />} />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.ProcurementWrite} />}
                      >
                        <Route
                          path="/inventory/purchase-orders/new"
                          element={<PurchaseOrderNewPage />}
                        />
                      </Route>
                      <Route
                        element={<RequirePermission permission={Permission.InventoryManage} />}
                      >
                        <Route path="/inventory/locations" element={<InventoryLocationsPage />} />
                        <Route path="/inventory/warehouse" element={<InventoryWarehousePage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.StatsRead} />}>
                        <Route
                          path="/finance/reconciliation"
                          element={<FinanceReconciliationPage />}
                        />
                        <Route path="/finance/reports" element={<FinanceReportPage />} />
                        <Route path="/finance/deposits" element={<FinanceDepositsPage />} />
                        <Route path="/finance/variance" element={<FinanceVariancePage />} />
                        <Route path="/activity-log" element={<ActivityLogPage />} />
                      </Route>
                      <Route element={<RequirePermission permission={Permission.SettingsRead} />}>
                        <Route path="/settings" element={<SettingsPage />} />
                      </Route>
                      <Route path="*" element={<NotFoundPage />} />
                    </Route>
                  </Routes>
                </Suspense>
              </RealtimeProvider>
            </QueryClientProvider>
          </StoreContext>
        </LocalizationProvider>
      </AppearanceProvider>
    </ErrorBoundary>
  );
}

export default App;
