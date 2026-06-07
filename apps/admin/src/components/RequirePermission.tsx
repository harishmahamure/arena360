import { toastUtils } from '@gaming-cafe/utils';
import { Box, Typography } from '@mui/material';
import { useEffect, useRef } from 'react';
import { Navigate, Outlet } from 'react-router-dom';
import { type Permission, usePermissions } from '../hooks/usePermissions';
import { getDefaultHomePath } from '../utils/homePath';

interface RequirePermissionProps {
  permission: Permission;
  redirectTo?: string;
}

function PermissionDeniedRedirect({
  redirectTo,
  permission,
}: {
  redirectTo: string;
  permission: Permission;
}) {
  const notified = useRef(false);

  useEffect(() => {
    if (notified.current) return;
    notified.current = true;
    toastUtils.warning(`You don't have access to this page (${permission}).`);
  }, [permission]);

  return <Navigate to={redirectTo} replace />;
}

export default function RequirePermission({ permission, redirectTo }: RequirePermissionProps) {
  const { can } = usePermissions();
  const fallbackPath = redirectTo ?? getDefaultHomePath(can);

  if (can(permission)) {
    return <Outlet />;
  }

  if (fallbackPath) {
    return <PermissionDeniedRedirect redirectTo={fallbackPath} permission={permission} />;
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
