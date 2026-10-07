'use client';

import { Paper, useTheme } from '@mui/material';
import type { ReactNode } from 'react';
import { type BreadcrumbItem, PageHeader } from './PageHeader';
import { PageShell, type PageShellProps } from './PageShell';

export interface FormPageProps {
  title: string;
  description?: string;
  backTo?: string;
  backLabel?: string;
  breadcrumbs?: BreadcrumbItem[];
  children: ReactNode;
  maxWidth?: PageShellProps['maxWidth'];
}

export function FormPage({
  title,
  description,
  backTo,
  backLabel,
  breadcrumbs,
  children,
  maxWidth = 1120,
}: FormPageProps) {
  const theme = useTheme();

  return (
    <PageShell maxWidth={maxWidth}>
      <PageHeader
        title={title}
        description={description}
        backTo={backTo}
        backLabel={backLabel}
        breadcrumbs={breadcrumbs}
      />
      <Paper
        elevation={0}
        sx={{
          p: { xs: 2, md: 3 },
          border: `1px solid ${theme.palette.divider}`,
          borderRadius: 1,
        }}
      >
        {children}
      </Paper>
    </PageShell>
  );
}
