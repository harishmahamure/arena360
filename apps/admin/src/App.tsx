import { Permission } from '@gaming-cafe/contracts';
import { Providers } from '@gaming-cafe/providers';
import { local } from '@gaming-cafe/utils';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useReducer } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';
import ErrorBoundary from './components/ErrorBoundary';
import RequirePermission from './components/RequirePermission';
import AuthLayout from './layouts/AuthLayout';
import DashboardLayout from './layouts/DashboardLayout';
import { RealtimeProvider } from './lib/realtime';
import LoginPage from './pages/auth/LoginPage';
import CashDepositsPage from './pages/dashboard/cash-deposits/CashDepositsPage';
import CashRegisterDetailPage from './pages/dashboard/cash-registers/CashRegisterDetailPage';
import CashRegistersPage from './pages/dashboard/cash-registers/CashRegistersPage';
import CreditPage from './pages/dashboard/credit/CreditPage';
import DashboardPage from './pages/dashboard/DashboardPage';
import DeviceDetailPage from './pages/dashboard/devices/DeviceDetailPage';
import DeviceNewPage from './pages/dashboard/devices/DeviceNewPage';
import DevicesPage from './pages/dashboard/devices/DevicesPage';
import ExpenseDetailPage from './pages/dashboard/expenses/ExpenseDetailPage';
import ExpenseNewPage from './pages/dashboard/expenses/ExpenseNewPage';
import ExpensesPage from './pages/dashboard/expenses/ExpensesPage';
import GameDetailPage from './pages/dashboard/games/GameDetailPage';
import GameNewPage from './pages/dashboard/games/GameNewPage';
import GamesPage from './pages/dashboard/games/GamesPage';
import InventoryLocationsPage from './pages/dashboard/inventory/InventoryLocationsPage';
import InventoryTransferNewPage from './pages/dashboard/inventory/InventoryTransferNewPage';
import InventoryTransfersPage from './pages/dashboard/inventory/InventoryTransfersPage';
import InventoryWarehousePage from './pages/dashboard/inventory/InventoryWarehousePage';
import InventoryWasteNewPage from './pages/dashboard/inventory/InventoryWasteNewPage';
import InventoryWastePage from './pages/dashboard/inventory/InventoryWastePage';
import InventoryWasteReportPage from './pages/dashboard/inventory/InventoryWasteReportPage';
import PlanTransactionDetailPage from './pages/dashboard/plan-transactions/PlanTransactionDetailPage';
import PlanTransactionNewPage from './pages/dashboard/plan-transactions/PlanTransactionNewPage';
import PlanTransactionsPage from './pages/dashboard/plan-transactions/PlanTransactionsPage';
import PlanDetailPage from './pages/dashboard/plans/PlanDetailPage';
import PlanNewPage from './pages/dashboard/plans/PlanNewPage';
import PlansPage from './pages/dashboard/plans/PlansPage';
import PlayerDetailPage from './pages/dashboard/players/PlayerDetailPage';
import PlayerNewPage from './pages/dashboard/players/PlayerNewPage';
import PlayersPage from './pages/dashboard/players/PlayersPage';
import ProductTransactionDetailPage from './pages/dashboard/product-transactions/ProductTransactionDetailPage';
import ProductTransactionNewPage from './pages/dashboard/product-transactions/ProductTransactionNewPage';
import ProductTransactionsPage from './pages/dashboard/product-transactions/ProductTransactionsPage';
import ProductDetailPage from './pages/dashboard/products/ProductDetailPage';
import ProductNewPage from './pages/dashboard/products/ProductNewPage';
import ProductsPage from './pages/dashboard/products/ProductsPage';
import SessionDetailPage from './pages/dashboard/sessions/SessionDetailPage';
import SessionNewPage from './pages/dashboard/sessions/SessionNewPage';
import SessionsPage from './pages/dashboard/sessions/SessionsPage';
import SettingsPage from './pages/dashboard/settings/SettingsPage';
import ShiftDetailPage from './pages/dashboard/shifts/ShiftDetailPage';
import ShiftsPage from './pages/dashboard/shifts/ShiftsPage';
import UnitDetailPage from './pages/dashboard/units/UnitDetailPage';
import UnitNewPage from './pages/dashboard/units/UnitNewPage';
import UnitsPage from './pages/dashboard/units/UnitsPage';
import VendorDetailPage from './pages/dashboard/vendors/VendorDetailPage';
import VendorNewPage from './pages/dashboard/vendors/VendorNewPage';
import VendorsPage from './pages/dashboard/vendors/VendorsPage';
import { StoreContext } from './store';
import { PERSIST_KEY } from './store/persistance';
import { rootInitialState, rootReducer } from './store/rootReducer';

const queryClient = new QueryClient();

function App() {
  const [state, dispatch] = useReducer(rootReducer, rootInitialState);

  useEffect(() => {
    local.set(PERSIST_KEY, JSON.stringify(state));
  }, [state]);

  return (
    <ErrorBoundary>
      <Providers>
        <StoreContext value={{ dispatch, state }}>
          <QueryClientProvider client={queryClient}>
            <RealtimeProvider>
              <Routes>
                <Route element={<AuthLayout />}>
                  <Route path="/login" element={<LoginPage />} />
                </Route>
                <Route element={<DashboardLayout />}>
                  <Route element={<RequirePermission permission={Permission.StatsRead} />}>
                    <Route path="/" element={<DashboardPage />} />
                  </Route>
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
                  <Route element={<RequirePermission permission={Permission.PlayerPlansRead} />}>
                    <Route path="/plan-transactions" element={<PlanTransactionsPage />} />
                    <Route path="/plan-transactions/:id" element={<PlanTransactionDetailPage />} />
                  </Route>
                  <Route element={<RequirePermission permission={Permission.PlayerPlansWrite} />}>
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
                  <Route element={<RequirePermission permission={Permission.TransactionsRead} />}>
                    <Route path="/product-transactions" element={<ProductTransactionsPage />} />
                    <Route
                      path="/product-transactions/:id"
                      element={<ProductTransactionDetailPage />}
                    />
                  </Route>
                  <Route element={<RequirePermission permission={Permission.TransactionsWrite} />}>
                    <Route
                      path="/product-transactions/new"
                      element={<ProductTransactionNewPage />}
                    />
                  </Route>
                  <Route path="/units" element={<UnitsPage />} />
                  <Route element={<RequirePermission permission={Permission.UnitsWrite} />}>
                    <Route path="/units/new" element={<UnitNewPage />} />
                  </Route>
                  <Route path="/units/:id" element={<UnitDetailPage />} />
                  <Route element={<RequirePermission permission={Permission.ShiftsRead} />}>
                    <Route path="/shifts" element={<ShiftsPage />} />
                    <Route path="/shifts/:id" element={<ShiftDetailPage />} />
                  </Route>
                  <Route element={<RequirePermission permission={Permission.CashRegistersRead} />}>
                    <Route path="/cash-registers" element={<CashRegistersPage />} />
                    <Route path="/cash-registers/:id" element={<CashRegisterDetailPage />} />
                  </Route>
                  <Route element={<RequirePermission permission={Permission.CashDepositsRead} />}>
                    <Route path="/cash-deposits" element={<CashDepositsPage />} />
                  </Route>
                  <Route element={<RequirePermission permission={Permission.CreditRead} />}>
                    <Route path="/credit" element={<CreditPage />} />
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
                    <Route path="/inventory/locations" element={<InventoryLocationsPage />} />
                    <Route path="/inventory/warehouse" element={<InventoryWarehousePage />} />
                    <Route path="/inventory/transfers" element={<InventoryTransfersPage />} />
                    <Route path="/inventory/transfers/new" element={<InventoryTransferNewPage />} />
                    <Route path="/inventory/waste" element={<InventoryWastePage />} />
                    <Route path="/inventory/waste/new" element={<InventoryWasteNewPage />} />
                    <Route path="/inventory/waste/report" element={<InventoryWasteReportPage />} />
                  </Route>
                  <Route element={<RequirePermission permission={Permission.ConfigRead} />}>
                    <Route path="/settings" element={<SettingsPage />} />
                  </Route>
                </Route>
                <Route path="*" element={<Navigate to="/" replace />} />
              </Routes>
            </RealtimeProvider>
          </QueryClientProvider>
        </StoreContext>
      </Providers>
    </ErrorBoundary>
  );
}

export default App;
