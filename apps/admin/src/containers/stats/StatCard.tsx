import { TrendingDown, TrendingUp } from '@mui/icons-material';
import { Box, Card, CardContent, Chip, Typography } from '@mui/material';
import type { ReactNode } from 'react';

interface StatCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  change?: {
    value: string | number;
    positive: boolean;
  };
  icon: ReactNode;
  color: string;
}

/**
 * Reusable stat card component for displaying dashboard metrics
 */
export function StatCard({ title, value, subtitle, change, icon, color }: StatCardProps) {
  return (
    <Card className="hover-lift" sx={{ height: '100%' }}>
      <CardContent>
        <Box
          sx={{
            display: 'flex',
            justifyContent: 'space-between',
            alignItems: 'flex-start',
            mb: 2,
          }}
        >
          <Box
            sx={{
              width: 48,
              height: 48,
              borderRadius: 2,
              bgcolor: `${color}15`,
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'center',
            }}
          >
            {icon}
          </Box>
          {change && (
            <Chip
              size="small"
              icon={change.positive ? <TrendingUp /> : <TrendingDown />}
              label={`${change.positive ? '+' : ''}${change.value}%`}
              color={change.positive ? 'success' : 'error'}
            />
          )}
        </Box>
        <Typography variant="h4" fontWeight={700} sx={{ mb: 0.5 }}>
          {value}
        </Typography>
        <Typography variant="body2" color="text.secondary" sx={{ mb: 0.5 }}>
          {title}
        </Typography>
        {subtitle && (
          <Typography variant="caption" color="text.secondary">
            {subtitle}
          </Typography>
        )}
      </CardContent>
    </Card>
  );
}
