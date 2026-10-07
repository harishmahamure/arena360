import { PageHeader, PageShell } from '@gaming-cafe/ui';
import { ArrowForward } from '@mui/icons-material';
import { Card, CardActionArea, Chip, Grid, Stack, Typography } from '@mui/material';
import { Link, useLocation } from 'react-router-dom';
import { analyticsDashboards } from '../../../constants/analyticsDashboards';

export default function AnalyticsIndexPage() {
  const location = useLocation();
  return (
    <PageShell
      header={
        <PageHeader
          title="Business dashboard"
          description="Choose a report to explore performance, customers, and opportunities. Each report has its own page and shares your applied date range."
        />
      }
    >
      <Grid container spacing={2} component="nav" aria-label="Business reports">
        {analyticsDashboards.map((dashboard) => (
          <Grid key={dashboard.id} size={{ xs: 12, md: 6, xl: 4 }}>
            <Card sx={{ height: '100%' }}>
              <CardActionArea
                component={Link}
                to={`/analytics/${dashboard.id}${location.search}`}
                aria-label={dashboard.title}
                sx={{ p: 3, height: '100%' }}
              >
                <Stack gap={2} sx={{ height: '100%', alignItems: 'start' }}>
                  <Chip size="small" label={dashboard.coverage} variant="outlined" />
                  <Typography component="h2" variant="h6">
                    {dashboard.title}
                  </Typography>
                  <Typography variant="body2" color="text.secondary" sx={{ flex: 1 }}>
                    {dashboard.action}
                  </Typography>
                  <Stack direction="row" gap={1} alignItems="center" color="primary.main">
                    <Typography variant="body2" fontWeight={600}>
                      Open report
                    </Typography>
                    <ArrowForward fontSize="small" />
                  </Stack>
                </Stack>
              </CardActionArea>
            </Card>
          </Grid>
        ))}
      </Grid>
    </PageShell>
  );
}
