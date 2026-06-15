'use client';

import { InputAdornment } from '@mui/material';
import { forwardRef, useState } from 'react';
import { filterCurrencyInput, formatDecimalDisplay } from '../../numericInputFilters';
import FormTextField, { type FormTextFieldProps } from './FormTextField';

export interface CurrencyFieldProps extends Omit<FormTextFieldProps, 'type'> {
  currencySymbol?: string;
  /** 'live' emits on every keystroke (string). 'blur' keeps draft locally until blur (FormBuilder). */
  commitMode?: 'live' | 'blur';
}

const CurrencyField = forwardRef<HTMLDivElement, CurrencyFieldProps>(
  (
    {
      currencySymbol = '₹',
      value,
      onChange,
      onBlur,
      inputProps,
      InputProps,
      commitMode = 'live',
      ...props
    },
    ref,
  ) => {
    const [draft, setDraft] = useState<string | null>(null);
    const editing = commitMode === 'blur' && draft !== null;
    const displayValue = editing
      ? draft
      : formatDecimalDisplay(value as string | number | null | undefined);

    const emitChange = (filtered: string) => {
      if (onChange) {
        onChange({ target: { value: filtered } } as React.ChangeEvent<HTMLInputElement>);
      }
    };

    return (
      <FormTextField
        ref={ref}
        type="text"
        value={displayValue}
        onFocus={(e) => {
          if (commitMode === 'blur') {
            setDraft(formatDecimalDisplay(value as string | number | null | undefined));
          }
          props.onFocus?.(e);
        }}
        onChange={(e) => {
          const filtered = filterCurrencyInput(e.target.value);
          if (commitMode === 'blur') {
            setDraft(filtered);
            return;
          }
          emitChange(filtered);
        }}
        onBlur={(e) => {
          if (commitMode === 'blur' && draft !== null) {
            emitChange(draft);
            setDraft(null);
          }
          onBlur?.(e);
        }}
        inputProps={{ inputMode: 'decimal', ...inputProps }}
        InputProps={{
          startAdornment: <InputAdornment position="start">{currencySymbol}</InputAdornment>,
          ...InputProps,
        }}
        {...props}
      />
    );
  },
);

CurrencyField.displayName = 'CurrencyField';

export default CurrencyField;
