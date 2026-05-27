import { Providers } from '@gaming-cafe/providers';
import { local } from '@gaming-cafe/utils';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { useEffect, useReducer } from 'react';
import { Navigate, Route, Routes } from 'react-router-dom';
import ErrorBoundary from './components/ErrorBoundary';
import AuthLayout from './layouts/AuthLayout';
import DashboardLayout from './layouts/DashboardLayout';
import LoginPage from './pages/auth/LoginPage';
import DashboardPage from './pages/dashboard/DashboardPage';
import DeviceGameDetailPage from './pages/dashboard/device-games/DeviceGameDetailPage';
import DeviceGameNewPage from './pages/dashboard/device-games/DeviceGameNewPage';
import DeviceGamesPage from './pages/dashboard/device-games/DeviceGamesPage';
import DeviceDetailPage from './pages/dashboard/devices/DeviceDetailPage';
import DeviceNewPage from './pages/dashboard/devices/DeviceNewPage';
import DevicesPage from './pages/dashboard/devices/DevicesPage';
import GameDetailPage from './pages/dashboard/games/GameDetailPage';
import GameNewPage from './pages/dashboard/games/GameNewPage';
import GamesPage from './pages/dashboard/games/GamesPage';
import MediaUploadPage from './pages/dashboard/media/MediaUploadPage';
import PlanTransactionDetailPage from './pages/dashboard/plan-transactions/PlanTransactionDetailPage';
import PlanTransactionNewPage from './pages/dashboard/plan-transactions/PlanTransactionNewPage';
import PlanTransactionsPage from './pages/dashboard/plan-transactions/PlanTransactionsPage';
import PlanDetailPage from './pages/dashboard/plans/PlanDetailPage';
import PlanNewPage from './pages/dashboard/plans/PlanNewPage';
import PlansPage from './pages/dashboard/plans/PlansPage';
import PlayerDetailPage from './pages/dashboard/players/PlayerDetailPage';
import PlayerNewPage from './pages/dashboard/players/PlayerNewPage';
import PlayersPage from './pages/dashboard/players/PlayersPage';
import ProductTransactionNewPage from './pages/dashboard/product-transactions/ProductTransactionNewPage';
import ProductTransactionsPage from './pages/dashboard/product-transactions/ProductTransactionsPage';
import ProductDetailPage from './pages/dashboard/products/ProductDetailPage';
import ProductNewPage from './pages/dashboard/products/ProductNewPage';
import ProductsPage from './pages/dashboard/products/ProductsPage';
import SessionDetailPage from './pages/dashboard/sessions/SessionDetailPage';
import SessionNewPage from './pages/dashboard/sessions/SessionNewPage';
import SessionsPage from './pages/dashboard/sessions/SessionsPage';
import UnitDetailPage from './pages/dashboard/units/UnitDetailPage';
import UnitNewPage from './pages/dashboard/units/UnitNewPage';
import UnitsPage from './pages/dashboard/units/UnitsPage';
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
            <Routes>
              <Route element={<AuthLayout />}>
                <Route path="/login" element={<LoginPage />} />
              </Route>
              <Route element={<DashboardLayout />}>
                <Route path="/" element={<DashboardPage />} />
                <Route path="/players" element={<PlayersPage />} />
                <Route path="/players/new" element={<PlayerNewPage />} />
                <Route path="/players/:id" element={<PlayerDetailPage />} />
                <Route path="/devices" element={<DevicesPage />} />
                <Route path="/devices/new" element={<DeviceNewPage />} />
                <Route path="/devices/:id" element={<DeviceDetailPage />} />
                <Route path="/games" element={<GamesPage />} />
                <Route path="/games/new" element={<GameNewPage />} />
                <Route path="/games/:id" element={<GameDetailPage />} />
                <Route path="/device-games" element={<DeviceGamesPage />} />
                <Route path="/device-games/new" element={<DeviceGameNewPage />} />
                <Route path="/device-games/:id" element={<DeviceGameDetailPage />} />
                <Route path="/plans" element={<PlansPage />} />
                <Route path="/plans/new" element={<PlanNewPage />} />
                <Route path="/plans/:id" element={<PlanDetailPage />} />
                <Route path="/plan-transactions" element={<PlanTransactionsPage />} />
                <Route path="/plan-transactions/new" element={<PlanTransactionNewPage />} />
                <Route path="/plan-transactions/:id" element={<PlanTransactionDetailPage />} />
                <Route path="/sessions" element={<SessionsPage />} />
                <Route path="/sessions/new" element={<SessionNewPage />} />
                <Route path="/sessions/:id" element={<SessionDetailPage />} />
                <Route path="/products" element={<ProductsPage />} />
                <Route path="/products/new" element={<ProductNewPage />} />
                <Route path="/products/:id" element={<ProductDetailPage />} />
                <Route path="/product-transactions" element={<ProductTransactionsPage />} />
                <Route path="/product-transactions/new" element={<ProductTransactionNewPage />} />
                <Route path="/units" element={<UnitsPage />} />
                <Route path="/units/new" element={<UnitNewPage />} />
                <Route path="/units/:id" element={<UnitDetailPage />} />
                <Route path="/media/upload" element={<MediaUploadPage />} />
              </Route>
              <Route path="*" element={<Navigate to="/" replace />} />
            </Routes>
          </QueryClientProvider>
        </StoreContext>
      </Providers>
    </ErrorBoundary>
  );
}

export default App;
