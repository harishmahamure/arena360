'use client';

import { formatTimeOfDay, parseTimeOfDay } from '@gaming-cafe/utils';
import { TimePicker } from '@mui/x-date-pickers/TimePicker';

export interface TimeOfDayFieldProps {
  value?: string;
  onChange: (value: string) => void;
  onBlur?: () => void;
  disabled?: boolean;
  error?: boolean;
  helperText?: string;
  placeholder?: string;
}

export function TimeOfDayField({
  value,
  onChange,
  onBlur,
  disabled,
  error,
  helperText,
  placeholder,
}: TimeOfDayFieldProps) {
  return (
    <TimePicker
      value={parseTimeOfDay(value)}
      onChange={(date) => onChange(formatTimeOfDay(date))}
      disabled={disabled}
      ampm={false}
      views={['hours', 'minutes', 'seconds']}
      format="HH:mm:ss"
      slotProps={{
        textField: {
          fullWidth: true,
          size: 'small',
          error,
          helperText,
          placeholder,
          onBlur,
        },
      }}
    />
  );
}
