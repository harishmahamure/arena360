'use client';

import { forwardRef, useState } from 'react';
import { filterDecimalInput, formatDecimalDisplay } from '../../numericInputFilters';
import FormTextField, { type FormTextFieldProps } from './FormTextField';

export interface DecimalFieldProps extends Omit<FormTextFieldProps, 'type'> {
  decimalPlaces?: number;
  allowNegative?: boolean;
  /** 'live' emits on every keystroke (string). 'blur' keeps draft locally until blur (FormBuilder). */
  commitMode?: 'live' | 'blur';
}

const DecimalField = forwardRef<HTMLDivElement, DecimalFieldProps>(
  (
    {
      value,
      onChange,
      onBlur,
      inputProps,
      decimalPlaces = 2,
      allowNegative = false,
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
          const filtered = filterDecimalInput(e.target.value, decimalPlaces, allowNegative);
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
        {...props}
      />
    );
  },
);

DecimalField.displayName = 'DecimalField';

export default DecimalField;
