import { PageShell } from '@gaming-cafe/ui';
import { ArrowBack } from '@mui/icons-material';
import { Box, Button } from '@mui/material';
import type { ReactNode } from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { GuidedForm, GuidedStep } from '../../components/GuidedForm';

export interface CounterSaleLayoutProps {
  backTo: string;
  backLabel: string;
  toolbar?: ReactNode;
  catalog: ReactNode;
  summary: ReactNode;
  alerts?: ReactNode;
  actions: ReactNode;
  busy: boolean;
  validateSelection: () => string | undefined;
  validatePayment: () => string | undefined;
  review: ReactNode;
}

export function CounterSaleLayout({
  backTo,
  backLabel,
  toolbar,
  catalog,
  summary,
  alerts,
  actions,
  busy,
  validateSelection,
  validatePayment,
  review,
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
      <GuidedForm busy={busy} actions={actions} review={review}>
        <GuidedStep title="Player & items" validate={validateSelection}>
          {catalog}
        </GuidedStep>
        <GuidedStep title="Cart & payment" validate={validatePayment}>
          {summary}
        </GuidedStep>
      </GuidedForm>
    </PageShell>
  );
}
