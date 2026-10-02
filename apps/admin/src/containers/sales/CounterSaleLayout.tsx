import { PageShell } from '@gaming-cafe/ui';
import { ArrowBack } from '@mui/icons-material';
import { Box, Button } from '@mui/material';
import type { ReactNode } from 'react';
import { Link as RouterLink } from 'react-router-dom';

export interface CounterSaleLayoutProps {
  backTo: string;
  backLabel: string;
  toolbar?: ReactNode;
  catalog: ReactNode;
  summary: ReactNode;
  alerts?: ReactNode;
  actions: ReactNode;
  busy: boolean;
}

/** One screen for counter sales: pick on the left, pay on the right. Submit handlers validate. */
export function CounterSaleLayout({
  backTo,
  backLabel,
  toolbar,
  catalog,
  summary,
  alerts,
  actions,
  busy,
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
      <Box
        component="fieldset"
        disabled={busy}
        sx={{
          m: 0,
          p: 0,
          border: 0,
          minWidth: 0,
          display: 'grid',
          gap: 3,
          gridTemplateColumns: { xs: '1fr', md: 'minmax(0, 7fr) minmax(320px, 5fr)' },
          alignItems: 'start',
        }}
      >
        <Box sx={{ minWidth: 0 }}>{catalog}</Box>
        <Box sx={{ minWidth: 0, position: { md: 'sticky' }, top: { md: 16 } }}>
          {summary}
          <Box sx={{ mt: 2 }}>{actions}</Box>
        </Box>
      </Box>
    </PageShell>
  );
}
