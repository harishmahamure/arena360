import {
  Box,
  Button,
  FormControlLabel,
  Switch,
  TextField,
  ToggleButton,
  ToggleButtonGroup,
} from '@mui/material';
import type { StatsDatePreset } from '../../hooks/useStatsDateRange';

const PRESETS: { value: StatsDatePreset; label: string }[] = [
  { value: 'today', label: 'Today' },
  { value: 'last7', label: '7 days' },
  { value: 'mtd', label: 'MTD' },
];

export interface StatsDateRangeToolbarProps {
  startDate: string;
  endDate: string;
  compare: boolean;
  onRangeChange: (startDate: string, endDate: string) => void;
  onCompareChange: (compare: boolean) => void;
  onPreset: (preset: StatsDatePreset) => void;
  onApply: () => void;
  isDirty?: boolean;
  showCompare?: boolean;
}

export function StatsDateRangeToolbar({
  startDate,
  endDate,
  compare,
  onRangeChange,
  onCompareChange,
  onPreset,
  onApply,
  isDirty = false,
  showCompare = true,
}: StatsDateRangeToolbarProps) {
  return (
    <Box
      sx={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: 2,
        alignItems: 'center',
        width: '100%',
      }}
    >
      <ToggleButtonGroup
        exclusive
        size="small"
        onChange={(_event, next: StatsDatePreset | null) => {
          if (next) onPreset(next);
        }}
        sx={{ flexWrap: 'wrap' }}
      >
        {PRESETS.map((option) => (
          <ToggleButton
            key={option.value}
            value={option.value}
            sx={{ flex: { xs: 1, sm: 'none' }, minWidth: { xs: 0, sm: 'auto' } }}
          >
            {option.label}
          </ToggleButton>
        ))}
      </ToggleButtonGroup>

      <TextField
        type="date"
        size="small"
        label="From"
        value={startDate}
        onChange={(e) => onRangeChange(e.target.value, endDate)}
        InputLabelProps={{ shrink: true }}
        sx={{ width: { xs: '100%', sm: 160 } }}
      />
      <TextField
        type="date"
        size="small"
        label="To"
        value={endDate}
        onChange={(e) => onRangeChange(startDate, e.target.value)}
        InputLabelProps={{ shrink: true }}
        sx={{ width: { xs: '100%', sm: 160 } }}
      />

      {showCompare && (
        <FormControlLabel
          control={
            <Switch
              size="small"
              checked={compare}
              onChange={(e) => onCompareChange(e.target.checked)}
            />
          }
          label="Compare previous period"
        />
      )}

      <Button variant="contained" size="small" onClick={onApply} disabled={!isDirty}>
        Apply
      </Button>
    </Box>
  );
}
