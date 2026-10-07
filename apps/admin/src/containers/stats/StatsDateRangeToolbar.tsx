import {
  Box,
  Button,
  FormControlLabel,
  Switch,
  ToggleButton,
  ToggleButtonGroup,
} from '@mui/material';
import { DatePicker } from '@mui/x-date-pickers/DatePicker';
import { format, isValid, parseISO } from 'date-fns';
import { useState } from 'react';
import { isStatsDate, isStatsRange, type StatsDatePreset } from '../../hooks/useStatsDateRange';

const PRESETS: { value: StatsDatePreset; label: string }[] = [
  { value: 'today', label: 'Today' },
  { value: 'last7', label: '7 days' },
  { value: 'last30', label: '30 days' },
  { value: 'mtd', label: 'This month' },
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
  const [openPicker, setOpenPicker] = useState<'start' | 'end' | null>(null);
  const dateValue = (value: string) => (isStatsDate(value) ? parseISO(value) : null);
  const dateString = (value: Date | null) =>
    value && isValid(value) ? format(value, 'yyyy-MM-dd') : '';
  const updateStart = (value: Date | null) => {
    const next = dateString(value);
    onRangeChange(next, next && (!isStatsDate(endDate) || next > endDate) ? next : endDate);
  };

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

      <DatePicker
        label="From"
        value={dateValue(startDate)}
        format="dd MMM yyyy"
        open={openPicker === 'start'}
        onOpen={() => setOpenPicker('start')}
        onClose={() => setOpenPicker((current) => (current === 'start' ? null : current))}
        closeOnSelect
        onChange={updateStart}
        onAccept={(value) => {
          if (value && isValid(value)) setOpenPicker('end');
        }}
        slotProps={{
          mobilePaper: { 'aria-label': 'From', 'aria-labelledby': undefined },
          textField: { size: 'small', sx: { width: { xs: '100%', sm: 190 } } },
          openPickerButton: { 'aria-label': 'Choose start date' },
        }}
      />
      <DatePicker
        label="To"
        value={dateValue(endDate)}
        minDate={dateValue(startDate) ?? undefined}
        format="dd MMM yyyy"
        open={openPicker === 'end'}
        onOpen={() => setOpenPicker('end')}
        onClose={() => setOpenPicker(null)}
        closeOnSelect
        onChange={(value) => onRangeChange(startDate, dateString(value))}
        slotProps={{
          mobilePaper: { 'aria-label': 'To', 'aria-labelledby': undefined },
          textField: {
            size: 'small',
            error: Boolean(startDate && endDate && !isStatsRange(startDate, endDate)),
            helperText:
              startDate && endDate && !isStatsRange(startDate, endDate)
                ? 'Choose an end date on or after the start'
                : undefined,
            sx: { width: { xs: '100%', sm: 190 } },
          },
          openPickerButton: { 'aria-label': 'Choose end date' },
        }}
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

      <Button
        variant="contained"
        size="small"
        onClick={onApply}
        disabled={!isDirty || !isStatsRange(startDate, endDate) || openPicker !== null}
      >
        Apply
      </Button>
    </Box>
  );
}
