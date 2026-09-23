'use client';

import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Chip,
  Divider,
  LinearProgress,
  Pagination,
  Paper,
  Stack,
  Step,
  StepLabel,
  Stepper,
  Tab,
  Tabs,
  TextField,
  Typography,
} from '@mui/material';
import { type ReactNode, useEffect } from 'react';
import { type Action, type Column, DataGrid } from './DataGrid';
import { PageHeader, type PageHeaderProps } from './PageHeader';
import { PageShell } from './PageShell';

export type StatusTone = 'default' | 'success' | 'warning' | 'error' | 'info';

export function StatusBadge({ label, tone = 'default' }: { label: string; tone?: StatusTone }) {
  return (
    <Chip
      size="small"
      label={label.replaceAll('_', ' ')}
      color={tone}
      sx={{ textTransform: 'capitalize' }}
    />
  );
}

export function MetricCard({
  label,
  value,
  detail,
  tone = 'default',
}: {
  label: string;
  value: ReactNode;
  detail?: ReactNode;
  tone?: StatusTone;
}) {
  const border = tone === 'default' ? 'divider' : `${tone}.main`;
  return (
    <Paper variant="outlined" sx={{ p: 2.5, minHeight: 120, borderTop: 3, borderTopColor: border }}>
      <Typography variant="body2" color="text.secondary">
        {label}
      </Typography>
      <Typography variant="h4" fontWeight={700} sx={{ mt: 0.5 }}>
        {value}
      </Typography>
      {detail ? (
        <Typography variant="caption" color="text.secondary">
          {detail}
        </Typography>
      ) : null}
    </Paper>
  );
}

export function OperationsShell({
  header,
  actions,
  children,
}: {
  header: PageHeaderProps;
  actions?: ReactNode;
  children: ReactNode;
}) {
  return (
    <PageShell
      header={
        <Stack direction={{ xs: 'column', md: 'row' }} justifyContent="space-between" gap={2}>
          <PageHeader {...header} />
          {actions ? <Box sx={{ pt: { md: 1 } }}>{actions}</Box> : null}
        </Stack>
      }
    >
      {children}
    </PageShell>
  );
}

export function ModuleTabs({
  value,
  onChange,
  tabs,
}: {
  value: string;
  onChange: (value: string) => void;
  tabs: Array<{ value: string; label: string }>;
}) {
  return (
    <Tabs
      value={value}
      onChange={(_, next) => onChange(next)}
      variant="scrollable"
      scrollButtons="auto"
      aria-label="Module views"
    >
      {tabs.map((tab) => (
        <Tab key={tab.value} value={tab.value} label={tab.label} />
      ))}
    </Tabs>
  );
}

export function CommandBar({ children }: { children: ReactNode }) {
  return (
    <Paper variant="outlined" sx={{ p: 1.5, mb: 2 }}>
      <Stack direction={{ xs: 'column', sm: 'row' }} gap={1.5}>
        {children}
      </Stack>
    </Paper>
  );
}

export const FilterBar = CommandBar;

export function WizardPage({
  title,
  steps,
  activeStep,
  children,
  onBack,
  onNext,
  nextLabel = 'Continue',
  nextDisabled,
  busy,
  error,
}: {
  title: string;
  steps: string[];
  activeStep: number;
  children: ReactNode;
  onBack?: () => void;
  onNext: () => void;
  nextLabel?: string;
  nextDisabled?: boolean;
  busy?: boolean;
  error?: string;
}) {
  return (
    <OperationsShell header={{ title }}>
      <Paper variant="outlined" sx={{ overflow: 'hidden' }}>
        {busy ? <LinearProgress /> : null}
        <Box sx={{ p: { xs: 2, md: 3 } }}>
          <Stepper activeStep={activeStep} alternativeLabel sx={{ mb: 4 }}>
            {steps.map((step) => (
              <Step key={step}>
                <StepLabel>{step}</StepLabel>
              </Step>
            ))}
          </Stepper>
          {error ? (
            <Alert severity="error" sx={{ mb: 2 }}>
              {error}
            </Alert>
          ) : null}
          <Box aria-live="polite">{children}</Box>
          <Divider sx={{ my: 3 }} />
          <Stack direction="row" justifyContent="space-between">
            <Button onClick={onBack} disabled={!onBack || busy}>
              Back
            </Button>
            <Button variant="contained" onClick={onNext} disabled={nextDisabled || busy}>
              {nextLabel}
            </Button>
          </Stack>
        </Box>
      </Paper>
    </OperationsShell>
  );
}

export function ActivityTimeline({
  items,
}: {
  items: Array<{ id: string; title: string; detail?: string; at: string }>;
}) {
  if (!items.length) return <Typography color="text.secondary">No activity yet.</Typography>;
  return (
    <Stack component="ol" spacing={2} sx={{ m: 0, pl: 2.5 }}>
      {items.map((item) => (
        <Box component="li" key={item.id}>
          <Typography fontWeight={600}>{item.title}</Typography>
          {item.detail ? <Typography variant="body2">{item.detail}</Typography> : null}
          <Typography variant="caption" color="text.secondary">
            {item.at}
          </Typography>
        </Box>
      ))}
    </Stack>
  );
}

export function AssistedForm({
  title,
  description,
  dirty = false,
  errors = [],
  children,
  actions,
}: {
  title: string;
  description?: string;
  dirty?: boolean;
  errors?: string[];
  children: ReactNode;
  actions: ReactNode;
}) {
  useEffect(() => {
    if (!dirty) return;
    const guard = (event: BeforeUnloadEvent) => event.preventDefault();
    window.addEventListener('beforeunload', guard);
    return () => window.removeEventListener('beforeunload', guard);
  }, [dirty]);
  return (
    <OperationsShell header={{ title, description }}>
      <Paper variant="outlined" sx={{ p: { xs: 2, md: 3 }, pb: 10 }}>
        {errors.length ? (
          <Alert severity="error" sx={{ mb: 2 }}>
            {errors.join(' · ')}
          </Alert>
        ) : null}
        {children}
      </Paper>
      <Paper elevation={4} sx={{ position: 'sticky', bottom: 0, p: 1.5, mt: -8, zIndex: 2 }}>
        <Stack direction="row" justifyContent="flex-end" gap={1}>
          {actions}
        </Stack>
      </Paper>
    </OperationsShell>
  );
}

export function EntityPicker<T>({
  label,
  options,
  value,
  getOptionLabel,
  onChange,
  required,
}: {
  label: string;
  options: T[];
  value: T | null;
  getOptionLabel: (option: T) => string;
  onChange: (value: T | null) => void;
  required?: boolean;
}) {
  return (
    <Autocomplete
      options={options}
      value={value}
      getOptionLabel={getOptionLabel}
      onChange={(_, next) => onChange(next)}
      renderInput={(params) => <TextField {...params} label={label} required={required} />}
    />
  );
}

export function LineItemEditor({
  children,
  onAdd,
  addLabel = 'Add line',
}: {
  children: ReactNode;
  onAdd: () => void;
  addLabel?: string;
}) {
  return (
    <Stack spacing={2}>
      {children}
      <Button onClick={onAdd} sx={{ alignSelf: 'flex-start' }}>
        {addLabel}
      </Button>
    </Stack>
  );
}

export function ApprovalQueue({
  items,
}: {
  items: Array<{ id: string; title: string; detail?: string; status?: string; action?: ReactNode }>;
}) {
  if (!items.length)
    return <Typography color="text.secondary">Nothing is waiting for approval.</Typography>;
  return (
    <Stack divider={<Divider flexItem />} spacing={1.5}>
      {items.map((item) => (
        <Stack key={item.id} direction="row" justifyContent="space-between" gap={2}>
          <Box>
            <Typography fontWeight={600}>{item.title}</Typography>
            {item.detail ? (
              <Typography variant="body2" color="text.secondary">
                {item.detail}
              </Typography>
            ) : null}
          </Box>
          <Stack direction="row" gap={1} alignItems="center">
            {item.status ? <StatusBadge label={item.status} tone="warning" /> : null}
            {item.action}
          </Stack>
        </Stack>
      ))}
    </Stack>
  );
}

export function ServerDataTable<T extends object>({
  rows,
  columns,
  actions,
  loading,
  page,
  totalPages,
  onPageChange,
  rowKey,
}: {
  rows: T[];
  columns: Column<T>[];
  actions?: Action<T>[];
  loading?: boolean;
  page: number;
  totalPages: number;
  onPageChange: (page: number) => void;
  rowKey: (row: T) => string | number;
}) {
  return (
    <Stack spacing={2}>
      {loading ? <LinearProgress /> : null}
      <DataGrid data={rows} columns={columns} actions={actions} rowKey={rowKey} />
      <Pagination
        page={page}
        count={totalPages}
        onChange={(_, next) => onPageChange(next)}
        sx={{ alignSelf: 'center' }}
      />
    </Stack>
  );
}

export function OperationState({
  kind,
  title,
  description,
  action,
}: {
  kind: 'loading' | 'empty' | 'unavailable' | 'permission-denied' | 'error';
  title: string;
  description?: string;
  action?: ReactNode;
}) {
  if (kind === 'loading')
    return (
      <Stack spacing={2}>
        <LinearProgress />
        <Typography color="text.secondary">{title}</Typography>
      </Stack>
    );
  const severity = kind === 'error' ? 'error' : kind === 'permission-denied' ? 'warning' : 'info';
  return (
    <Alert severity={severity} action={action}>
      {title}
      {description ? <Typography variant="body2">{description}</Typography> : null}
    </Alert>
  );
}
