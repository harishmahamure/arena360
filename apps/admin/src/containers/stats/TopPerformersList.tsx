import { Box, Card, CardContent, CardHeader, Chip, Divider, Typography } from '@mui/material';

interface TopPerformerItem {
  id: string;
  name: string;
  primaryMetric: string | number;
  secondaryMetric: string | number;
}

interface TopPerformersListProps {
  title: string;
  items: TopPerformerItem[];
  primaryLabel?: string;
  secondaryLabel: string;
}

/**
 * Reusable component for displaying top performers (plans, games, players)
 */
export function TopPerformersList({
  title,
  items,
  primaryLabel,
  secondaryLabel,
}: TopPerformersListProps) {
  return (
    <Card variant="outlined">
      <CardHeader title={title} />
      <Divider />
      <CardContent>
        {items.map((item, index) => (
          <Box
            key={item.id}
            sx={{
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
              mb: 2,
              '&:last-child': { mb: 0 },
            }}
          >
            <Box sx={{ display: 'flex', alignItems: 'center', gap: 1 }}>
              <Chip
                label={index + 1}
                size="small"
                color={index < 3 ? 'primary' : 'default'}
                sx={{ minWidth: 32 }}
              />
              <Box>
                <Typography variant="body2" fontWeight={500}>
                  {item.name}
                </Typography>
                <Typography variant="caption" color="text.secondary">
                  {secondaryLabel}: {item.secondaryMetric}
                </Typography>
              </Box>
            </Box>
            <Typography variant="body2" fontWeight={600}>
              {primaryLabel ? `${primaryLabel}: ` : ''}
              {item.primaryMetric}
            </Typography>
          </Box>
        ))}
        {items.length === 0 && (
          <Typography variant="body2" color="text.secondary" textAlign="center">
            No data available
          </Typography>
        )}
      </CardContent>
    </Card>
  );
}
