'use client';

import { ArrowBack } from '@mui/icons-material';
import {
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  type ChipProps,
  Stack,
  Typography,
} from '@mui/material';
import type { ReactNode } from 'react';
import { Link as RouterLink } from 'react-router-dom';
import { ErrorPanel } from './ErrorPanel';
import { FormSkeleton } from './FormSkeleton';
import { type BreadcrumbItem, PageHeader } from './PageHeader';
import { PageShell, type PageShellProps } from './PageShell';

export interface DetailPageStatus {
  label: string;
  color?: ChipProps['color'];
}

export interface DetailPageSection {
  title: string;
  description?: string;
  content: ReactNode;
}

export interface DetailPageProps {
  title: string;
  description?: string;
  backTo?: string;
  backLabel?: string;
  breadcrumbs?: BreadcrumbItem[];
  status?: DetailPageStatus;
  isLoading?: boolean;
  error?: string | null;
  onRetry?: () => void;
  /** Inline alerts between status and summary (e.g. mutation errors) */
  banner?: ReactNode;
  summary?: ReactNode;
  sections?: DetailPageSection[];
  actions?: ReactNode;
  maxWidth?: PageShellProps['maxWidth'];
}

export function DetailPage({
  title,
  description,
  backTo,
  backLabel = 'Back',
  breadcrumbs,
  status,
  isLoading = false,
  error,
  onRetry,
  banner,
  summary,
  sections,
  actions,
  maxWidth = false,
}: DetailPageProps) {
  if (isLoading) {
    return (
      <PageShell maxWidth={maxWidth}>
        <FormSkeleton />
      </PageShell>
    );
  }

  if (error) {
    return (
      <PageShell maxWidth={maxWidth}>
        <ErrorPanel message={error} onRetry={onRetry} />
        {backTo && (
          <Button
            component={RouterLink}
            to={backTo}
            startIcon={<ArrowBack />}
            sx={{ mt: 2, ml: -1 }}
          >
            {backLabel}
          </Button>
        )}
      </PageShell>
    );
  }

  const hasSections = sections && sections.length > 0;

  return (
    <PageShell maxWidth={maxWidth}>
      <PageHeader
        title={title}
        description={description}
        backTo={backTo}
        backLabel={backLabel}
        breadcrumbs={breadcrumbs}
      />

      {status && <Chip label={status.label} color={status.color} size="small" sx={{ mb: 3 }} />}

      {banner}

      {summary && (
        <Card variant="outlined" sx={{ mb: 3 }}>
          <CardContent>{summary}</CardContent>
        </Card>
      )}

      {actions && (
        <Stack direction="row" spacing={2} sx={{ mb: hasSections ? 3 : 0 }}>
          {actions}
        </Stack>
      )}

      {sections?.map((section) => (
        <Box key={section.title} sx={{ mb: 3 }}>
          <Typography variant="h6" fontWeight={600} gutterBottom={!!section.description}>
            {section.title}
          </Typography>
          {section.description && (
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              {section.description}
            </Typography>
          )}
          {section.content}
        </Box>
      ))}
    </PageShell>
  );
}
