import { ArrowBack } from '@mui/icons-material';
import { Box, Breadcrumbs, Button, Link, Typography } from '@mui/material';
import { Link as RouterLink } from 'react-router-dom';

export interface BreadcrumbItem {
  label: string;
  to?: string;
}

interface PageHeaderProps {
  title: string;
  description?: string;
  backTo?: string;
  backLabel?: string;
  breadcrumbs?: BreadcrumbItem[];
}

export function PageHeader({
  title,
  description,
  backTo,
  backLabel = 'Back',
  breadcrumbs,
}: PageHeaderProps) {
  return (
    <Box sx={{ mb: 3 }}>
      {backTo && (
        <Button
          component={RouterLink}
          to={backTo}
          startIcon={<ArrowBack />}
          sx={{ mb: 1, ml: -1 }}
        >
          {backLabel}
        </Button>
      )}
      {breadcrumbs && breadcrumbs.length > 0 && (
        <Breadcrumbs sx={{ mb: 1 }}>
          {breadcrumbs.map((item, index) => {
            const isLast = index === breadcrumbs.length - 1;
            if (isLast || !item.to) {
              return (
                <Typography key={item.label} variant="body2" color="text.secondary">
                  {item.label}
                </Typography>
              );
            }
            return (
              <Link
                key={item.label}
                component={RouterLink}
                to={item.to}
                underline="hover"
                color="inherit"
                variant="body2"
              >
                {item.label}
              </Link>
            );
          })}
        </Breadcrumbs>
      )}
      <Typography variant="h4" fontWeight={600} gutterBottom={!!description}>
        {title}
      </Typography>
      {description && (
        <Typography variant="body2" color="text.secondary">
          {description}
        </Typography>
      )}
    </Box>
  );
}
