import { Box, Chip, Typography } from '@mui/material';
import Autocomplete from '@mui/material/Autocomplete';
import CircularProgress from '@mui/material/CircularProgress';
import TextField from '@mui/material/TextField';
import { useQuery } from '@tanstack/react-query';
import * as React from 'react';
import { type Control, Controller, type FieldValues } from 'react-hook-form';

export interface SearchOption {
  id: string | number;
  label: string;
}

interface Props<T extends SearchOption> {
  name: string;
  control: Control<FieldValues>;

  label?: string;
  placeholder?: string;

  onSearch: (query: string) => Promise<T[]>;
  disabled?: boolean;
  queryKey?: string;
  helperText?: string;
  multiple?: boolean; // Enable multiple selection mode
  onSearchComplete?: (data: SearchOption) => void;
}

export function RHFSearchOnEnterAutocomplete<T extends SearchOption>({
  name,
  control,
  label = 'Search',
  placeholder = 'Start typing to search...',
  onSearch,
  disabled,
  queryKey = 'search',
  helperText,
  multiple = false,
  onSearchComplete,
}: Props<T>) {
  const [inputValue, setInputValue] = React.useState('');
  const [debouncedValue, setDebouncedValue] = React.useState('');
  const [open, setOpen] = React.useState(false);
  const [selectedOption, setSelectedOption] = React.useState<T | null>(null);
  const [selectedOptions, setSelectedOptions] = React.useState<T[]>([]);
  // Debounce the input value
  React.useEffect(() => {
    const timer = setTimeout(() => {
      setDebouncedValue(inputValue.trim());
    }, 300);
    return () => clearTimeout(timer);
  }, [inputValue]);

  const { data: options = [], isLoading } = useQuery({
    queryKey: [queryKey, debouncedValue],
    queryFn: () => onSearch(debouncedValue),
    enabled: debouncedValue.length >= 2,
  });

  React.useEffect(() => {
    if (debouncedValue.length >= 2 && (options.length > 0 || isLoading)) {
      setOpen(true);
    }
  }, [options, isLoading, debouncedValue]);

  return (
    <Controller
      name={name}
      control={control}
      render={({ field, fieldState }) => (
        <Box>
          <Autocomplete
            multiple={true}
            disabled={disabled}
            options={options}
            loading={isLoading}
            open={open}
            onOpen={() => {
              if (debouncedValue.length >= 2) {
                setOpen(true);
              }
            }}
            onClose={() => setOpen(false)}
            filterOptions={(x) => x}
            getOptionLabel={(o) => (typeof o === 'string' ? o : o.label)}
            isOptionEqualToValue={(option, value) => option.id === value?.id}
            value={multiple ? selectedOptions : selectedOption ? [selectedOption] : []}
            onChange={(_, value) => {
              const options = value as T[];

              if (multiple) {
                // Multiple select mode
                setSelectedOptions(options);
                // For productIds field, return array of objects with productId and quantity
                if (name === 'productIds') {
                  field.onChange(
                    options.map((opt) => ({
                      productId: opt.id,
                      quantity: 1,
                    })),
                  );
                } else {
                  field.onChange(options.map((opt) => opt.id));
                }
              } else {
                // Single select mode - limit to 1 selection
                const option = options.at(-1) ?? null;
                setSelectedOption(option);
                field.onChange(option?.id ?? null);
                if (option && onSearchComplete) {
                  onSearchComplete(option);
                }
                setOpen(false);
              }
              setInputValue('');
            }}
            inputValue={inputValue}
            onInputChange={(_, v, reason) => {
              if (reason === 'input') {
                setInputValue(v);
                // Clear selection if user starts typing again (single mode only)
                if (!multiple && selectedOption && v !== selectedOption.label) {
                  setSelectedOption(null);
                  field.onChange(null);
                }
                if (v.trim().length < 2) {
                  setOpen(false);
                }
              }
              if (reason === 'clear') {
                setInputValue('');
                if (multiple) {
                  setSelectedOptions([]);
                  // For productIds field, ensure we clear with empty array
                  field.onChange([]);
                } else {
                  setSelectedOption(null);
                  field.onChange(null);
                }
                setOpen(false);
              }
            }}
            renderTags={(value, getTagProps) => {
              if (!multiple) {
                // Single select mode - show only one chip
                return value.slice(0, 1).map((option, index) => (
                  <Chip
                    {...getTagProps({ index })}
                    key={option.id}
                    label={option.label}
                    size="small"
                    onDelete={() => {
                      setSelectedOption(null);
                      field.onChange(null);
                      setInputValue('');
                    }}
                  />
                ));
              }
              // Multiple select mode - default behavior
              return value.map((option, index) => (
                <Chip
                  {...getTagProps({ index })}
                  key={option.id}
                  label={option.label}
                  size="small"
                />
              ));
            }}
            renderInput={(params) => (
              <TextField
                {...params}
                label={label}
                placeholder={
                  (!multiple && selectedOption) || (multiple && selectedOptions.length > 0)
                    ? ''
                    : placeholder
                }
                error={!!fieldState.error}
                helperText={fieldState.error?.message}
                InputProps={{
                  ...params.InputProps,
                  endAdornment: (
                    <>
                      {isLoading ? <CircularProgress size={18} /> : null}
                      {params.InputProps.endAdornment}
                    </>
                  ),
                }}
              />
            )}
          />
          {helperText && (
            <Typography variant="caption" color="text.secondary" display="block" sx={{ mt: 0.5 }}>
              {helperText}
            </Typography>
          )}
        </Box>
      )}
    />
  );
}
