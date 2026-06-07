import { Home, PlayCircle } from '@mui/icons-material';
import { Box, Button, Typography } from '@mui/material';
import { Link as RouterLink } from 'react-router-dom';
import { usePermissions } from '../hooks/usePermissions';
import { getDefaultHomePath } from '../utils/homePath';

export default function NotFoundPage() {
  const { can, isStaff } = usePermissions();
  const homePath = getDefaultHomePath(can);

  return (
    <Box
      sx={{
        px: 4,
        py: 8,
        maxWidth: 480,
        mx: 'auto',
        textAlign: 'center',
      }}
    >
      <Typography variant="h3" fontWeight={700} gutterBottom>
        Page not found
      </Typography>
      <Typography variant="body1" color="text.secondary" sx={{ mb: 4 }}>
        The page you are looking for does not exist or may have been moved.
      </Typography>
      <Box sx={{ display: 'flex', gap: 2, justifyContent: 'center', flexWrap: 'wrap' }}>
        <Button component={RouterLink} to={homePath} variant="contained" startIcon={<Home />}>
          {isStaff ? 'Staff dashboard' : 'Dashboard'}
        </Button>
        <Button
          component={RouterLink}
          to="/sessions?active=true"
          variant="outlined"
          startIcon={<PlayCircle />}
        >
          Active sessions
        </Button>
      </Box>
    </Box>
  );
}
