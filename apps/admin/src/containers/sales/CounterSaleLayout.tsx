import { PageShell } from '@gaming-cafe/ui';
import { ArrowBack } from '@mui/icons-material';
import { Box, Button, Grid } from '@mui/material';
import type { ReactNode } from 'react';
import { Link as RouterLink } from 'react-router-dom';

export interface CounterSaleLayoutProps {
  backTo: string;
  backLabel: string;
  toolbar?: ReactNode;
  catalog: ReactNode;
  summary: ReactNode;
  alerts?: ReactNode;
}

export function CounterSaleLayout({
  backTo,
  backLabel,
  toolbar,
  catalog,
  summary,
  alerts,
}: CounterSaleLayoutProps) {
  return (
    <PageShell
      header={
        <Box sx={{ mb: 2 }}>
          <Button component={RouterLink} to={backTo} startIcon={<ArrowBack />} sx={{ ml: -1 }}>
            {backLabel}
          </Button>
        </Box>
      }
      toolbar={toolbar}
    >
      {alerts}
      <Grid container spacing={3}>
        <Grid size={{ xs: 12, md: 7 }}>{catalog}</Grid>
        <Grid size={{ xs: 12, md: 5 }}>
          <Box
            sx={{
              position: { md: 'sticky' },
              top: { md: 80 },
              alignSelf: 'flex-start',
            }}
          >
            {summary}
          </Box>
        </Grid>
      </Grid>
    </PageShell>
  );
}
