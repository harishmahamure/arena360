import { Permission } from '@gaming-cafe/contracts';
import { LinearProgress } from '@mui/material';
import { Navigate } from 'react-router-dom';
import { useOnboarding } from '../../hooks/useOnboarding';
import { usePermissions } from '../../hooks/usePermissions';
import { getDefaultHomePath } from '../../utils/homePath';
import AdminDashboardView from './AdminDashboardView';
import StaffDashboardView from './StaffDashboardView';

export default function DashboardPage() {
  const { can } = usePermissions();
  const setup = useOnboarding();
  if (setup.canWrite && setup.isPending)
    return <LinearProgress aria-label="Checking venue setup" />;
  if (
    setup.canWrite &&
    setup.data &&
    !setup.data.progress.startedAt &&
    !setup.data.progress.completedAt
  ) {
    return <Navigate to="/onboarding" replace />;
  }
  if (!can(Permission.StatsRead)) {
    const home = getDefaultHomePath(can);
    return home === '/' ? (
      <div role="status">
        No modules are assigned. Ask your access administrator to assign a role.
      </div>
    ) : (
      <Navigate to={home} replace />
    );
  }
  return can(Permission.FinanceRead) ? <AdminDashboardView /> : <StaffDashboardView />;
}
