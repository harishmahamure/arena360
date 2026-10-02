import { usePermissions } from '../../hooks/usePermissions';
import AdminDashboardView from './AdminDashboardView';
import StaffDashboardView from './StaffDashboardView';

export default function DashboardPage() {
  const { isStaff } = usePermissions();
  return isStaff ? <StaffDashboardView /> : <AdminDashboardView />;
}
