import { AuthLayout as BaseAuthLayout } from '@gaming-cafe/ui';
import { Outlet } from 'react-router-dom';

export default function AuthLayout() {
  return (
    <BaseAuthLayout>
      <Outlet />
    </BaseAuthLayout>
  );
}
