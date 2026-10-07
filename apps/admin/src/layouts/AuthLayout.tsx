import {
  ArrowOutward,
  LockOutlined,
  SpaceDashboardOutlined,
  TuneRounded,
} from '@mui/icons-material';
import { Box, LinearProgress, Typography } from '@mui/material';
import { Suspense, useRef } from 'react';
import { Outlet, useLocation } from 'react-router-dom';
import { BrandMark } from '../components/BrandMark';
import { useAutoFocusFirstField } from '../hooks/useAutoFocusFirstField';
import { useBranding } from '../services/config/branding';

export default function AuthLayout() {
  const location = useLocation();
  const brand = useBranding();
  const formRef = useRef<HTMLDivElement>(null);
  useAutoFocusFirstField(formRef, location.pathname, true);
  return (
    <Box className="auth-layout">
      <Box className="auth-story">
        <div className="workspace-brand">
          <BrandMark />
        </div>
        <Box className="auth-story-copy">
          <Typography variant="overline" sx={{ color: '#9fb6aa' }}>
            YOUR VENUE. ONE WORKSPACE.
          </Typography>
          <Typography component="h1">
            Great experiences
            <br />
            start behind
            <br />
            the scenes<span>.</span>
          </Typography>
          <Typography className="auth-description">
            Keep your floor running, your team in sync, and your business moving forward.
          </Typography>
          <Box className="auth-capability">
            <SpaceDashboardOutlined />
            <div>
              <strong>See the whole picture</strong>
              <small>Sessions, sales, and operations in one place.</small>
            </div>
            <ArrowOutward fontSize="small" />
          </Box>
          <Box className="auth-capability">
            <TuneRounded />
            <div>
              <strong>Make it work your way</strong>
              <small>Clear policies for every venue and location.</small>
            </div>
            <ArrowOutward fontSize="small" />
          </Box>
        </Box>
        <Typography variant="caption" sx={{ color: '#90a59a' }}>
          {brand.name} · Built for the way you operate.
        </Typography>
      </Box>
      <Box className="auth-form-side">
        <Box className="auth-form-card" ref={formRef}>
          <Box className="auth-security">
            <LockOutlined sx={{ fontSize: 15 }} /> SECURE WORKSPACE ACCESS
          </Box>
          <Suspense key={location.pathname} fallback={<LinearProgress aria-label="Loading page" />}>
            <Outlet />
          </Suspense>
          <Typography
            variant="caption"
            color="text.secondary"
            sx={{ display: 'block', textAlign: 'center', mt: 4 }}
          >
            Need access? Contact your organization administrator.
          </Typography>
        </Box>
      </Box>
    </Box>
  );
}
