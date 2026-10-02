import { Navigate } from 'react-router-dom';
import { Permission, usePermissions } from '../../hooks/usePermissions';
import { getDefaultHomePath } from '../../utils/homePath';
import AdminDashboardView from './AdminDashboardView';
import StaffDashboardView from './StaffDashboardView';

export default function DashboardPage() {
  const { can } = usePermissions();
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
