import { Card, CardActionArea, CardContent, Typography } from '@mui/material';
import type { PlanResponse } from '../../services/plans/list';

export interface PlanSaleCardProps {
  plan: PlanResponse;
  selected: boolean;
  onSelect: (plan: PlanResponse) => void;
  disabled?: boolean;
}

export function PlanSaleCard({ plan, selected, onSelect, disabled = false }: PlanSaleCardProps) {
  return (
    <Card
      variant="outlined"
      sx={{
        height: '100%',
        borderColor: selected ? 'primary.main' : 'divider',
        borderWidth: selected ? 2 : 1,
        bgcolor: selected ? 'action.selected' : 'background.paper',
      }}
    >
      <CardActionArea
        disabled={disabled}
        onClick={() => onSelect(plan)}
        sx={{ minHeight: 44, height: '100%', alignItems: 'stretch' }}
      >
        <CardContent sx={{ p: 2 }}>
          <Typography variant="body2" fontWeight={600} noWrap>
            {plan.name}
          </Typography>
          <Typography variant="caption" color="text.secondary" display="block">
            {plan.planType}
          </Typography>
          <Typography variant="body2" fontWeight={600} sx={{ mt: 1 }}>
            ₹{parseFloat(plan.price).toFixed(2)}
          </Typography>
          {plan.timeCredits != null && (
            <Typography variant="caption" color="text.secondary" display="block">
              {plan.timeCredits} min credits
            </Typography>
          )}
          <Typography variant="caption" color="text.secondary" display="block">
            Valid {plan.validityDays} days
          </Typography>
          {plan.deviceType && (
            <Typography variant="caption" color="text.secondary" display="block">
              {plan.deviceType}
              {plan.deviceSubType ? ` · ${plan.deviceSubType}` : ''}
            </Typography>
          )}
        </CardContent>
      </CardActionArea>
    </Card>
  );
}
