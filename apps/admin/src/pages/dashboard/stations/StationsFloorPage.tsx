import { PageShell } from '@gaming-cafe/ui';
import { Typography } from '@mui/material';
import { StationsFloorView } from './StationsFloorView';

export default function StationsFloorPage() {
  return (
    <PageShell>
      <Typography variant="h5" gutterBottom>
        Stations
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
        Live floor view — in-use sessions, free stations by type, and maintenance status.
      </Typography>
      <StationsFloorView />
    </PageShell>
  );
}
