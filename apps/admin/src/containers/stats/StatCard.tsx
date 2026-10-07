import { TrendingDown, TrendingUp } from '@mui/icons-material';
import { Box, Card, Chip, Typography } from '@mui/material';
import { alpha, useTheme } from '@mui/material/styles';
import type { ReactNode } from 'react';
export type StatTone = 'success' | 'info' | 'warning' | 'error' | 'primary';
interface StatCardProps {
  title: string;
  value: string | number;
  subtitle?: string;
  change?: { value: string | number; positive: boolean };
  icon: ReactNode;
  tone: StatTone;
  shade?: 'main' | 'dark' | 'light';
}
export function StatCard({
  title,
  value,
  subtitle,
  change,
  icon,
  tone,
  shade = 'main',
}: StatCardProps) {
  const theme = useTheme();
  const color = theme.palette[tone][shade];
  return (
    <Card sx={{ p: 2.5, height: '100%' }}>
      <Box sx={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', mb: 2 }}>
        <Typography fontSize={12} color="text.secondary" fontWeight={550}>
          {title}
        </Typography>
        <Box
          sx={{
            display: 'flex',
            p: 0.75,
            borderRadius: 1.5,
            color,
            bgcolor: alpha(color, 0.07),
            '& svg': { fontSize: 18 },
          }}
        >
          {icon}
        </Box>
      </Box>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, flexWrap: 'wrap' }}>
        <Typography
          sx={{ fontSize: 28, fontWeight: 650, letterSpacing: '-.04em', lineHeight: 1.2 }}
        >
          {value}
        </Typography>
        {change && Number.isFinite(Number(change.value)) && (
          <Chip
            icon={change.positive ? <TrendingUp /> : <TrendingDown />}
            label={`${change.positive ? '+' : ''}${change.value}%`}
            color={change.positive ? 'success' : 'error'}
            size="small"
          />
        )}
      </Box>
      {subtitle && (
        <Typography
          variant="caption"
          color="text.secondary"
          sx={{ display: 'block', mt: 1.5, whiteSpace: 'pre-line', fontSize: 10 }}
        >
          {subtitle}
        </Typography>
      )}
    </Card>
  );
}
