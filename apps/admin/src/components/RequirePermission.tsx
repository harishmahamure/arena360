import { Box, Typography } from '@mui/material';
import { Navigate, Outlet } from 'react-router-dom';
import { type Permission, usePermissions } from '../hooks/usePermissions';

interface RequirePermissionProps {
  permission: Permission;
  redirectTo?: string;
}

export default function RequirePermission({
  permission,
  redirectTo = '/sessions',
}: RequirePermissionProps) {
  const { can } = usePermissions();

  if (can(permission)) {
    return <Outlet />;
  }

  if (redirectTo) {
    return <Navigate to={redirectTo} replace />;
  }

  return (
    <Box sx={{ px: 4, py: 6 }}>
      <Typography variant="h5" fontWeight={600} gutterBottom>
        Access denied
      </Typography>
      <Typography variant="body2" color="text.secondary">
        You do not have permission to view this page.
      </Typography>
    </Box>
  );
}
