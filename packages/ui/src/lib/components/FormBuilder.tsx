'use client';

import { yupResolver } from '@hookform/resolvers/yup';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Chip,
  Collapse,
  Divider,
  GridLegacy as Grid,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useCallback, useEffect, useMemo, useState } from 'react';
import {
  type Control,
  Controller,
  type DefaultValues,
  type FieldErrors,
  type FieldValues,
  type Path,
  type UseFormReturn,
  type UseFormWatch,
  useForm,
} from 'react-hook-form';
import type * as yup from 'yup';
import { parseDecimalFieldValue } from '../numericInputFilters';
import CurrencyField from './forms/CurrencyField';
import DecimalField from './forms/DecimalField';
import FileUpload from './forms/FileUpload';
import FormButton from './forms/FormButton';
import FormCheckbox from './forms/FormCheckbox';
import FormContainer from './forms/FormContainer';
import FormRadioGroup, { type FormRadioOption } from './forms/FormRadioGroup';
import FormSelect, { type FormSelectOption } from './forms/FormSelect';
import FormSwitch from './forms/FormSwitch';
import FormTextField from './forms/FormTextField';
import IntegerField from './forms/IntegerField';
import OtpField from './forms/OtpField';
import PasswordField from './forms/PasswordField';
import PhoneField from './forms/PhoneField';
import { RHFSearchOnEnterAutocomplete, type SearchOption } from './forms/SearchInput';
import UsernameField from './forms/UsernameField';
import { WizardProgress } from './WizardProgress';

export type FieldType =
  | 'text'
  | 'email'
  | 'number'
  | 'currency'
  | 'password'
  | 'textarea'
  | 'select'
  | 'checkbox'
  | 'switch'
  | 'radio'
  | 'file'
  | 'date'
  | 'time'
  | 'datetime'
  | 'hidden'
  | 'custom'
  | 'search'
  | 'multiselect'
  | 'phone'
  | 'otp'
  | 'username';

export type FormMode = 'add' | 'edit' | 'view';

export interface FieldConfig<T extends FieldValues = FieldValues> {
  /** Unique field name - must match form data key */
  name: Path<T>;
  /** Field label */
  label: string;
  /** Field type */
  type: FieldType;
  /** Placeholder text */
  placeholder?: string;
  /** Helper text shown below the field */
  helperText?: string;
  /** If true, field is required */
  required?: boolean;
  /** If true, field is disabled */
  disabled?: boolean;
  /** If true, field spans full width (12 cols) */
  fullWidth?: boolean;
  /** Grid column span (1-12) */
  gridCols?: number;
  /** Custom render already provides its own visible field label */
  hideCustomLabel?: boolean;
  /** Options for select, radio, checkbox group */
  options?: FormSelectOption[] | FormRadioOption[];
  /** If true, multiline textarea for text type */
  multiline?: boolean;
  /** Number of rows for multiline */
  rows?: number;
  /** Min value for number input */
  min?: number;
  /** Max value for number input */
  max?: number;
  /** Step for number input (1 implies integer keyboard) */
  step?: number | string;
  /** If true, number field uses integer input (digits only) */
  integer?: boolean;
  /** Max decimal places for number/currency fields */
  decimalPlaces?: number;
  /** Accept attribute for file upload */
  accept?: string;
  /** Allow multiple files */
  multiple?: boolean;
  /** Max file size in bytes */
  maxSize?: number;
  render?: (props: {
    field: ControllerRenderProps;
    fieldState: { error?: { message?: string }; isDirty: boolean };
    form: UseFormReturn<T>;
    disabled: boolean;
  }) => React.ReactNode;
  visible?: (values: T) => boolean;
  validate?: (value: unknown, context: T) => string | undefined;
  /** Show only in specific modes */
  showInModes?: FormMode[];
  /** Additional props passed to the field component */
  fieldProps?: Record<string, unknown>;
  /** Search handler for search field type - returns options */
  onSearch?: (query: string) => Promise<SearchOption[] | undefined>;
  /** Query key for search field caching */
  searchQueryKey?: string;

  /**
   * @deprecated Use `helperText` instead. Kept for backward compatibility.
   */
  formHelperText?: string;
}

function FieldHelperCaption({ text }: { text?: string }) {
  if (!text) return null;
  return (
    <Typography
      variant="caption"
      color="text.secondary"
      sx={{ display: 'block', mt: 0.5, px: 0.2 }}
    >
      {text}
    </Typography>
  );
}

interface ControllerRenderProps {
  onChange: (...event: unknown[]) => void;
  onBlur: () => void;
  value: unknown;
  name: string;
  ref: React.Ref<unknown>;
}

export interface FormSection<T extends FieldValues = FieldValues> {
  /** Section title */
  title?: string;
  /** Section description */
  description?: string;
  /** Fields in this section */
  fields: FieldConfig<T>[];
  /** Grid column span for the section */
  gridCols?: number;
  /** Show divider after section */
  showDivider?: boolean;
}

export interface FormBuilderProps<T extends FieldValues = FieldValues> {
  /** Form configuration - either flat fields or sections */
  fields?: FieldConfig<T>[];
  /** Sections for grouped fields */
  sections?: FormSection<T>[];
  /** Guided entry with validated steps and a final review. Opt-in for existing consumers. */
  wizard?: boolean;
  /** Field names grouped into meaningful steps; unassigned fields appear in Details. */
  wizardSteps?: { title: string; description?: string; fields: Path<T>[] }[];
  /** Yup validation schema */
  schema?: yup.AnyObjectSchema;
  /** Default form values */
  defaultValues?: DefaultValues<T>;
  /** Form mode: add, edit, or view */
  mode?: FormMode;
  /** Submit handler */
  onSubmit?: (data: T, dirtyFields: Partial<Record<keyof T, boolean>>) => void | Promise<void>;
  /** Cancel handler */
  onCancel?: () => void;
  /** Reset handler */
  onReset?: () => void;
  /** Called when form values change */
  onChange?: (values: T, dirtyFields: Partial<Record<keyof T, boolean>>) => void;
  /** Submit button text */
  submitLabel?: string;
  /** Cancel button text */
  cancelLabel?: string;
  /** Reset button text */
  resetLabel?: string;
  /** If true, shows loading state on submit */
  loading?: boolean;
  /** If true, shows success checkmark on submit button */
  submitSuccess?: boolean;
  /** Label on submit button when `submitSuccess` is true */
  submitSuccessLabel?: string;
  /** If true, shows error state on submit button */
  submitError?: boolean;
  /** Label on submit button when `submitError` is true */
  submitErrorLabel?: string;
  /** If true, shows cancel button */
  showCancel?: boolean;
  /** If true, shows reset button */
  showReset?: boolean;
  /** If true, disables submit when form is not dirty (for edit mode) */
  requireDirty?: boolean;
  /** Form error message */
  error?: string;
  /** Success message */
  success?: string;
  /** Grid spacing */
  spacing?: number;
  /** Button alignment */
  buttonAlign?: 'left' | 'center' | 'right';
  /** Custom form instance (for external control) */
  form?: UseFormReturn<T>;
  /** Custom actions to render in button area */
  actions?: React.ReactNode;
  /** Container props */
  containerProps?: Omit<React.ComponentProps<typeof Box>, 'onSubmit' | 'ref'>;

  /** Callback when search is complete */
  onSearchComplete?: (data: SearchOption) => void;
}

function getDirtyValues<T extends FieldValues>(
  dirtyFields: Partial<Record<keyof T, boolean | object>>,
  values: T,
): Partial<T> {
  const dirtyValues: Partial<T> = {};

  Object.keys(dirtyFields).forEach((key) => {
    const isDirty = dirtyFields[key as keyof T];
    if (isDirty) {
      dirtyValues[key as keyof T] = values[key as keyof T];
    }
  });

  return dirtyValues;
}

interface FieldRendererProps<T extends FieldValues> {
  config: FieldConfig<T>;
  control: UseFormReturn<T>['control'];
  errors: FieldErrors<T>;
  mode: FormMode;
  watch: UseFormWatch<T>;
  form: UseFormReturn<T>;
  onSearchComplete?: (data: SearchOption) => void;
}

function FieldRenderer<T extends FieldValues>({
  config,
  control,
  errors,
  mode,
  watch,
  form,
  onSearchComplete,
}: FieldRendererProps<T>) {
  const {
    name,
    label,
    type,
    placeholder,
    helperText,
    required,
    disabled,
    options = [],
    multiline,
    rows = 4,
    min,
    max,
    step,
    integer,
    decimalPlaces,
    accept,
    multiple,
    maxSize,
    render: customRender,
    visible,
    showInModes,
    fieldProps = {},
    onSearch,
    searchQueryKey,
    formHelperText,
  } = config;

  const watchedValues = watch();
  const fieldError = errors[name];
  const errorMessage = fieldError?.message as string | undefined;
  const isViewMode = mode === 'view';
  const isDisabled = disabled || isViewMode;

  if (visible && !visible(watchedValues as T)) {
    return null;
  }

  if (showInModes && !showInModes.includes(mode)) {
    return null;
  }

  const resolvedHelper = errorMessage || helperText || formHelperText;

  const commonProps = {
    fullWidth: true,
    error: !!errorMessage,
    helperText: resolvedHelper,
    disabled: isDisabled,
    required,
    placeholder,
    ...fieldProps,
  };

  return (
    <Box>
      <Controller
        name={name}
        control={control}
        rules={{
          required: required ? `${label} is required` : false,
          validate: config.validate
            ? (value) => config.validate?.(value, form.getValues()) || true
            : undefined,
        }}
        render={({ field, fieldState }) => {
          // Custom render
          if (type === 'custom' && customRender) {
            return (
              <>
                {customRender({
                  field,
                  fieldState: {
                    error: fieldState.error,
                    isDirty: fieldState.isDirty,
                  },
                  form,
                  disabled: isDisabled,
                })}
              </>
            );
          }

          if (type === 'hidden') {
            return <input type="hidden" {...field} />;
          }

          switch (type) {
            case 'text':
            case 'email':
            case 'date':
            case 'time':
            case 'datetime':
              return (
                <FormTextField
                  {...field}
                  inputRef={field.ref}
                  {...commonProps}
                  label={label}
                  required={required}
                  autoComplete="one-time-code"
                  type={type === 'datetime' ? 'datetime-local' : type}
                  multiline={multiline}
                  rows={multiline ? rows : undefined}
                  value={field.value ?? ''}
                  inputProps={{
                    autoComplete: 'one-time-code',
                  }}
                  InputLabelProps={{ shrink: true }}
                />
              );

            case 'currency':
              return (
                <CurrencyField
                  {...commonProps}
                  label={label}
                  autoComplete="one-time-code"
                  InputLabelProps={{ shrink: true }}
                  commitMode="blur"
                  inputRef={field.ref}
                  name={field.name}
                  value={field.value ?? ''}
                  onBlur={field.onBlur}
                  onChange={(e) => {
                    field.onChange(parseDecimalFieldValue(e.target.value));
                  }}
                />
              );

            case 'number': {
              const useInteger = integer === true || step === 1 || step === '1';
              const numericValue = field.value ?? '';

              if (useInteger) {
                return (
                  <IntegerField
                    {...field}
                    inputRef={field.ref}
                    {...commonProps}
                    label={label}
                    autoComplete="one-time-code"
                    InputLabelProps={{ shrink: true }}
                    inputProps={{ min, max }}
                    value={numericValue}
                    onChange={(e) => {
                      const value = e.target.value;
                      field.onChange(value === '' ? '' : Number(value));
                    }}
                  />
                );
              }

              return (
                <DecimalField
                  {...commonProps}
                  label={label}
                  autoComplete="one-time-code"
                  InputLabelProps={{ shrink: true }}
                  decimalPlaces={decimalPlaces}
                  commitMode="blur"
                  inputRef={field.ref}
                  name={field.name}
                  inputProps={{ min, max }}
                  value={numericValue}
                  onBlur={field.onBlur}
                  onChange={(e) => {
                    field.onChange(parseDecimalFieldValue(e.target.value));
                  }}
                />
              );
            }

            case 'textarea':
              return (
                <FormTextField
                  {...field}
                  inputRef={field.ref}
                  {...commonProps}
                  label={label}
                  multiline
                  rows={rows}
                  value={field.value ?? ''}
                  autoComplete="one-time-code"
                  inputProps={{ autoComplete: 'one-time-code' }}
                />
              );

            case 'password':
              return (
                <PasswordField
                  {...field}
                  inputRef={field.ref}
                  {...commonProps}
                  label={label}
                  value={field.value ?? ''}
                  autoComplete="new-password"
                  inputProps={{
                    autoComplete: 'off',
                  }}
                  InputLabelProps={{ shrink: true }}
                />
              );

            case 'phone':
              return (
                <PhoneField
                  {...field}
                  inputRef={field.ref}
                  {...commonProps}
                  label={label}
                  required={required}
                  value={field.value ?? ''}
                  InputLabelProps={{ shrink: true }}
                />
              );

            case 'otp':
              return (
                <OtpField
                  {...field}
                  inputRef={field.ref}
                  {...commonProps}
                  label={label}
                  required={required}
                  value={field.value ?? ''}
                  InputLabelProps={{ shrink: true }}
                />
              );

            case 'username':
              return (
                <UsernameField
                  {...field}
                  inputRef={field.ref}
                  {...commonProps}
                  label={label}
                  required={required}
                  value={field.value ?? ''}
                  InputLabelProps={{ shrink: true }}
                />
              );

            case 'select':
              return (
                <FormSelect
                  {...commonProps}
                  label={label}
                  options={options as FormSelectOption[]}
                  value={field.value ?? ''}
                  onChange={(e) => field.onChange(e.target.value)}
                  onBlur={field.onBlur}
                  name={field.name}
                  inputRef={field.ref}
                />
              );

            case 'multiselect': {
              const msOptions = (options as FormSelectOption[]) || [];
              const selectedValues: (string | number)[] = Array.isArray(field.value)
                ? field.value
                : [];
              const selectedObjects = selectedValues
                .map((v) => msOptions.find((o) => String(o.value) === String(v)))
                .filter(Boolean) as FormSelectOption[];
              return (
                <Autocomplete
                  multiple
                  options={msOptions}
                  getOptionLabel={(o) => o.label}
                  isOptionEqualToValue={(a, b) => String(a.value) === String(b.value)}
                  value={selectedObjects}
                  onChange={(_e, newValue) => {
                    field.onChange(newValue.map((o) => o.value));
                  }}
                  disabled={isDisabled}
                  renderTags={(tagValue, getTagProps) =>
                    tagValue.map((option, index) => (
                      <Chip
                        size="small"
                        label={option.label}
                        {...getTagProps({ index })}
                        key={String(option.value)}
                      />
                    ))
                  }
                  renderInput={(params) => (
                    <TextField
                      {...params}
                      label={label}
                      placeholder={placeholder}
                      error={!!errorMessage}
                      helperText={resolvedHelper}
                      size="small"
                    />
                  )}
                />
              );
            }

            case 'checkbox':
              return (
                <>
                  <FormCheckbox
                    {...commonProps}
                    label={label}
                    checked={!!field.value}
                    onChange={(e) => field.onChange(e.target.checked)}
                    onBlur={field.onBlur}
                    name={field.name}
                    inputRef={field.ref}
                  />
                  <FieldHelperCaption
                    text={!errorMessage ? helperText || formHelperText : undefined}
                  />
                </>
              );

            case 'switch':
              return (
                <>
                  <FormSwitch
                    {...commonProps}
                    label={label + (required ? ' *' : '')}
                    checked={!!field.value}
                    onChange={(e) => field.onChange(e.target.checked)}
                    onBlur={field.onBlur}
                    name={field.name}
                    inputRef={field.ref}
                  />
                  <FieldHelperCaption
                    text={!errorMessage ? helperText || formHelperText : undefined}
                  />
                </>
              );

            case 'radio':
              return (
                <>
                  <FormRadioGroup
                    {...commonProps}
                    label={label}
                    options={options as FormRadioOption[]}
                    value={field.value ?? ''}
                    onChange={(e) => field.onChange(e.target.value)}
                    onBlur={field.onBlur}
                    name={field.name}
                  />
                  <FieldHelperCaption
                    text={!errorMessage ? helperText || formHelperText : undefined}
                  />
                </>
              );

            case 'file':
              return (
                <Box>
                  <Typography variant="body2" color="text.secondary" sx={{ mb: 1 }}>
                    {label}
                    {required && ' *'}
                  </Typography>
                  <FileUpload
                    accept={accept}
                    multiple={multiple}
                    maxSize={maxSize}
                    files={field.value || []}
                    onChange={(files) => field.onChange(files)}
                    onRemove={(index) => {
                      const currentFiles = field.value || [];
                      const newFiles = currentFiles.filter((_: File, i: number) => i !== index);
                      field.onChange(newFiles);
                    }}
                    error={!!errorMessage}
                    errorMessage={errorMessage}
                    helperText={helperText}
                    disabled={isDisabled}
                  />
                </Box>
              );

            case 'search':
              return (
                <RHFSearchOnEnterAutocomplete
                  name={name}
                  control={control as Control<FieldValues>}
                  label={label}
                  placeholder={placeholder}
                  onSearch={onSearch as (query: string) => Promise<SearchOption[]>}
                  disabled={isDisabled}
                  queryKey={searchQueryKey}
                  helperText={resolvedHelper}
                  multiple={multiple}
                  onSearchComplete={onSearchComplete}
                />
              );

            default:
              return (
                <FormTextField
                  {...field}
                  inputRef={field.ref}
                  {...commonProps}
                  label={label}
                  value={field.value ?? ''}
                  autoComplete="one-time-code"
                  inputProps={{ autoComplete: 'one-time-code' }}
                />
              );
          }
        }}
      />
    </Box>
  );
}

export function FormBuilder<T extends FieldValues = FieldValues>({
  fields = [],
  sections = [],
  schema,
  defaultValues,
  mode = 'add',
  wizard = false,
  wizardSteps,
  onSubmit,
  onCancel,
  onReset,
  onChange,
  submitLabel,
  cancelLabel = 'Cancel',
  resetLabel = 'Reset',
  loading = false,
  submitSuccess = false,
  submitSuccessLabel,
  submitError = false,
  submitErrorLabel,
  showCancel = false,
  showReset = false,
  requireDirty = true,
  error,
  success,
  spacing = 3,
  buttonAlign = 'right',
  form: externalForm,
  actions,
  containerProps,
  onSearchComplete,
}: FormBuilderProps<T>) {
  const [activeStep, setActiveStep] = useState(0);
  const [stepError, setStepError] = useState('');
  const [advancing, setAdvancing] = useState(false);
  const [searchLabels, setSearchLabels] = useState<Record<string, SearchOption>>({});
  // Create internal form if not provided externally
  const internalForm = useForm<T>({
    defaultValues,
    resolver: schema ? yupResolver(schema) : undefined,
    mode: 'onChange',
  });

  const form = externalForm || internalForm;

  const {
    control,
    handleSubmit,
    formState: { errors, isDirty, dirtyFields, isSubmitting },
    watch,
    reset,
  } = form;

  // Computed values
  const watchedValues = watch();

  const isViewMode = mode === 'view';
  const isEditMode = mode === 'edit';
  const isAddMode = mode === 'add';

  // Determine submit button label
  const computedSubmitLabel = useMemo(() => {
    if (submitLabel) return submitLabel;
    if (isAddMode) return 'Create';
    if (isEditMode) return 'Update';
    return 'Submit';
  }, [submitLabel, isAddMode, isEditMode]);

  // Check if submit should be disabled
  const isSubmitDisabled = useMemo(() => {
    if (isViewMode) return true;
    if (loading || isSubmitting || submitSuccess) return true;
    if (isEditMode && requireDirty && !isDirty) return true;
    return false;
  }, [isViewMode, loading, isSubmitting, submitSuccess, isEditMode, requireDirty, isDirty]);

  // Handle form submission
  const handleFormSubmit = useCallback(
    async (data: T) => {
      if (onSubmit) {
        const dirty = dirtyFields as Partial<Record<keyof T, boolean>>;
        await onSubmit(data, dirty);
      }
    },
    [onSubmit, dirtyFields],
  );

  // Handle reset
  const handleReset = useCallback(() => {
    reset(defaultValues);
    setActiveStep(0);
    setStepError('');
    onReset?.();
  }, [reset, defaultValues, onReset]);

  // Watch for changes and notify
  useEffect(() => {
    if (onChange) {
      const dirty = dirtyFields as Partial<Record<keyof T, boolean>>;
      onChange(watchedValues as T, dirty);
    }
  }, [watchedValues, dirtyFields, onChange]);

  // Async-loaded edit forms need reset when defaultValues arrive after mount.
  useEffect(() => {
    if ((isEditMode || isViewMode) && defaultValues) {
      reset(defaultValues);
    }
  }, [defaultValues, reset, isEditMode, isViewMode]);

  // Combine fields from both flat and sections
  const allFields = useMemo(() => {
    if (sections.length > 0) {
      return sections.flatMap((section) => section.fields);
    }
    return fields;
  }, [fields, sections]);

  const isVisible = (field: FieldConfig<T>) =>
    field.type !== 'hidden' &&
    (!field.showInModes || field.showInModes.includes(mode)) &&
    (!field.visible || field.visible(watchedValues as T));
  const steps = (() => {
    if (wizardSteps?.length) {
      const assigned = new Set(wizardSteps.flatMap((step) => step.fields));
      const configured = wizardSteps.map((step) => ({
        ...step,
        fields: allFields.filter((field) => step.fields.includes(field.name) && isVisible(field)),
      }));
      const remaining = allFields.filter((field) => !assigned.has(field.name) && isVisible(field));
      return [
        ...configured,
        ...(remaining.length ? [{ title: 'Additional details', fields: remaining }] : []),
      ].filter((step) => step.fields.length);
    }
    if (sections.length)
      return sections
        .map((section, index) => ({
          ...section,
          title: section.title || `Details ${index + 1}`,
          fields: section.fields.filter(isVisible),
        }))
        .filter((section) => section.fields.length);
    const visible = allFields.filter(isVisible);
    return [{ title: 'Details', fields: visible }];
  })();
  const guided = wizard && !isViewMode;
  const currentStep = Math.min(activeStep, steps.length);
  const reviewing = guided && currentStep === steps.length;
  const busy = loading || isSubmitting || advancing || submitSuccess;
  const getValue = (name: string, values: unknown = watchedValues): unknown =>
    name
      .split('.')
      .reduce<unknown>(
        (value, key) =>
          value && typeof value === 'object' ? (value as Record<string, unknown>)[key] : undefined,
        values,
      );
  const nextStep = async () => {
    if (busy) return;
    setAdvancing(true);
    const valid = await form.trigger(
      steps[currentStep]?.fields.map((field) => field.name),
      { shouldFocus: true },
    );
    setAdvancing(false);
    if (!valid) {
      setStepError('Complete the highlighted fields to continue.');
      return;
    }
    setStepError('');
    setActiveStep(currentStep + 1);
  };
  const onInvalid = (invalid: FieldErrors<T>) => {
    const index = steps.findIndex((step) =>
      step.fields.some((field) => getValue(field.name, invalid)),
    );
    if (guided && index >= 0) {
      setActiveStep(index);
      const field = steps[index]?.fields.find((field) => getValue(field.name, invalid));
      if (field) setTimeout(() => form.setFocus(field.name), 0);
    }
    setStepError('Check the highlighted fields before saving.');
  };
  const reviewValue = (field: FieldConfig<T>) => {
    const value = getValue(field.name);
    if (value === undefined || value === null || value === '') return 'Not provided';
    if (field.type === 'password' || field.type === 'otp') return '••••••••';
    if (field.type === 'search' && searchLabels[field.name]?.id === value)
      return searchLabels[field.name]?.label;
    if (typeof value === 'boolean') return value ? 'Enabled' : 'Disabled';
    const format = (item: unknown): string => {
      const option = field.options?.find((option) => String(option.value) === String(item));
      if (option) return String(option.label);
      if (item instanceof File) return item.name;
      if (item && typeof item === 'object') {
        const record = item as Record<string, unknown>;
        return String(record.label ?? record.name ?? record.username ?? 'Selected');
      }
      return String(item);
    };
    return Array.isArray(value) ? value.map(format).join(', ') || 'None selected' : format(value);
  };

  // Render fields with grid
  const renderFields = (fieldsToRender: FieldConfig<T>[], inactive = false) => (
    <Grid container spacing={spacing}>
      {fieldsToRender
        .filter((field) => field.type === 'hidden' || isVisible(field))
        .map((fieldConfig) => {
          if (fieldConfig.type === 'hidden') {
            return (
              <FieldRenderer
                key={fieldConfig.name}
                config={inactive ? { ...fieldConfig, disabled: true } : fieldConfig}
                control={control}
                errors={errors}
                mode={mode}
                watch={watch}
                form={form}
                onSearchComplete={(option) => {
                  setSearchLabels((previous) => ({ ...previous, [fieldConfig.name]: option }));
                  onSearchComplete?.(option);
                }}
              />
            );
          }

          const gridCols = fieldConfig.fullWidth ? 12 : fieldConfig.gridCols || 6;

          return (
            <Grid item xs={12} sm={gridCols} key={fieldConfig.name} component="div">
              {fieldConfig.type === 'custom' && !fieldConfig.hideCustomLabel && (
                <Typography variant="body2" color="text.secondary" sx={{ pb: 1, px: 0.2 }}>
                  {fieldConfig.label}
                  {fieldConfig.required && <span style={{ color: 'red' }}>*</span>}
                </Typography>
              )}
              <FieldRenderer
                config={inactive ? { ...fieldConfig, disabled: true } : fieldConfig}
                control={control}
                errors={errors}
                mode={mode}
                watch={watch}
                form={form}
                onSearchComplete={(option) => {
                  setSearchLabels((previous) => ({ ...previous, [fieldConfig.name]: option }));
                  onSearchComplete?.(option);
                }}
              />
            </Grid>
          );
        })}
    </Grid>
  );

  // Render sections
  const renderSections = () => (
    <Stack spacing={4}>
      {sections.map((section, index) => (
        <Box key={section.title ?? section.fields.map((field) => field.name).join('-')}>
          {section.title && (
            <Typography variant="h6" sx={{ mb: 1, fontWeight: 600 }}>
              {section.title}
            </Typography>
          )}
          {section.description && (
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              {section.description}
            </Typography>
          )}
          {renderFields(section.fields)}
          {section.showDivider && index < sections.length - 1 && <Divider sx={{ mt: 3 }} />}
        </Box>
      ))}
    </Stack>
  );

  // Button alignment styles
  const buttonAlignStyles = {
    left: 'flex-start',
    center: 'center',
    right: 'flex-end',
  };

  return (
    <FormContainer
      {...containerProps}
      onSubmit={(event) => {
        if (document.activeElement instanceof HTMLElement) document.activeElement.blur();
        if (guided && !reviewing) {
          event.preventDefault();
          void nextStep();
        } else if (!busy && !isSubmitDisabled)
          void handleSubmit(handleFormSubmit, onInvalid)(event);
      }}
      sx={{ p: { xs: 2, sm: 3 }, bgcolor: 'background.paper', borderRadius: 2 }}
      style={{
        width: '100%',
        maxWidth: '100%',
        ...containerProps?.style,
      }}
    >
      <Collapse in={!!error}>
        <Alert severity="error" sx={{ mb: 3 }}>
          {error}
        </Alert>
      </Collapse>

      <Collapse in={!!success}>
        <Alert severity="success" sx={{ mb: 3 }}>
          {success}
        </Alert>
      </Collapse>

      {guided && (
        <WizardProgress
          titles={[...steps.map((step) => step.title), 'Review & confirm']}
          activeStep={currentStep}
          disabled={busy}
          onBackTo={(step) => {
            setActiveStep(step);
            setStepError('');
          }}
        />
      )}
      {stepError && (
        <Alert severity="error" sx={{ mb: 2 }}>
          {stepError}
        </Alert>
      )}
      <Box
        component="fieldset"
        disabled={busy}
        sx={{ mb: 3, p: 0, m: 0, border: 0, minWidth: 0, pointerEvents: busy ? 'none' : undefined }}
      >
        {guided && (
          <>
            <Box hidden>{renderFields(allFields.filter((field) => field.type === 'hidden'))}</Box>
            {steps.map((step, index) => (
              <Box
                key={step.title}
                hidden={reviewing || currentStep !== index}
                sx={{ '&[hidden]': { display: 'none' } }}
              >
                {step.description && (
                  <Typography color="text.secondary" sx={{ mb: 3 }}>
                    {step.description}
                  </Typography>
                )}
                {renderFields(step.fields, reviewing || currentStep !== index)}
              </Box>
            ))}
          </>
        )}
        {guided ? (
          reviewing ? (
            <Stack spacing={3}>
              <Typography color="text.secondary">
                Check your details before you confirm. Nothing is saved until you submit.
              </Typography>
              {steps.map((step, index) => (
                <Box
                  key={step.title}
                  sx={{ border: 1, borderColor: 'divider', borderRadius: 2, p: 2 }}
                >
                  <Stack
                    direction="row"
                    justifyContent="space-between"
                    alignItems="center"
                    sx={{ mb: 1 }}
                  >
                    <Typography fontWeight={600}>{step.title}</Typography>
                    <Button
                      type="button"
                      disabled={busy}
                      onClick={() => setActiveStep(index)}
                      aria-label={`Edit ${step.title}`}
                    >
                      Edit
                    </Button>
                  </Stack>
                  <Box
                    component="dl"
                    sx={{
                      m: 0,
                      display: 'grid',
                      gridTemplateColumns: { xs: '1fr', sm: '1fr 1fr' },
                      gap: 1.5,
                    }}
                  >
                    {step.fields
                      .filter(
                        (field) =>
                          field.name !== 'confirmPassword' &&
                          (field.type !== 'custom' || getValue(field.name) !== undefined),
                      )
                      .map((field) => (
                        <Box key={field.name} sx={{ minWidth: 0 }}>
                          <Typography component="dt" variant="caption" color="text.secondary">
                            {field.label}
                          </Typography>
                          <Typography
                            component="dd"
                            variant="body2"
                            sx={{ m: 0, overflowWrap: 'anywhere' }}
                          >
                            {reviewValue(field)}
                          </Typography>
                        </Box>
                      ))}
                  </Box>
                </Box>
              ))}
            </Stack>
          ) : null
        ) : sections.length > 0 ? (
          renderSections()
        ) : (
          renderFields(allFields)
        )}
      </Box>

      {isEditMode && isDirty && (
        <Box sx={{ mb: 2 }}>
          <Typography variant="caption" color="warning.main">
            Unsaved changes:{' '}
            {allFields
              .filter((field) => getValue(field.name, dirtyFields))
              .map((field) => field.label)
              .join(', ')}
          </Typography>
        </Box>
      )}

      {!isViewMode && (
        <Box
          sx={{
            display: 'flex',
            justifyContent: buttonAlignStyles[buttonAlign],
            gap: 2,
            mt: 2,
            flexWrap: 'wrap',
            pt: 2,
            borderTop: 1,
            borderColor: 'divider',
          }}
        >
          {showCancel && (
            <FormButton
              type="button"
              variant="outlined"
              color="inherit"
              onClick={onCancel}
              disabled={loading || isSubmitting}
            >
              {cancelLabel}
            </FormButton>
          )}

          {showReset && (
            <FormButton
              type="button"
              variant="outlined"
              color="secondary"
              onClick={handleReset}
              disabled={loading || isSubmitting || !isDirty}
            >
              {resetLabel}
            </FormButton>
          )}

          {guided && currentStep > 0 && (
            <FormButton
              type="button"
              variant="outlined"
              disabled={busy}
              onClick={() => {
                setActiveStep(currentStep - 1);
                setStepError('');
              }}
            >
              Back
            </FormButton>
          )}
          {(!guided || reviewing) && actions}

          <FormButton
            type="submit"
            variant="contained"
            color="primary"
            loading={loading || isSubmitting}
            success={submitSuccess}
            successLabel={submitSuccessLabel}
            error={submitError}
            errorLabel={submitErrorLabel}
            disabled={guided && !reviewing ? busy : isSubmitDisabled}
          >
            {guided && !reviewing
              ? currentStep === steps.length - 1
                ? 'Review details'
                : 'Continue'
              : computedSubmitLabel}
          </FormButton>
        </Box>
      )}
    </FormContainer>
  );
}

// ============================================================================
// Hook for External Form Control
// ============================================================================

export interface UseFormBuilderOptions<T extends FieldValues> {
  schema?: yup.AnyObjectSchema;
  defaultValues?: DefaultValues<T>;
  mode?: 'onChange' | 'onBlur' | 'onSubmit' | 'all';
}

export function useFormBuilder<T extends FieldValues>(options: UseFormBuilderOptions<T> = {}) {
  const { schema, defaultValues, mode = 'onChange' } = options;

  const form = useForm<T>({
    defaultValues,
    resolver: schema ? yupResolver(schema) : undefined,
    mode,
  });

  const getDirtyFieldValues = useCallback(() => {
    return getDirtyValues(
      form.formState.dirtyFields as Partial<Record<keyof T, boolean | object>>,
      form.getValues(),
    );
  }, [form]);

  const hasDirtyFields = useCallback(() => {
    return Object.keys(form.formState.dirtyFields).length > 0;
  }, [form]);

  return {
    form,
    getDirtyFieldValues,
    hasDirtyFields,
    isDirty: form.formState.isDirty,
    isValid: form.formState.isValid,
    isSubmitting: form.formState.isSubmitting,
    errors: form.formState.errors,
    dirtyFields: form.formState.dirtyFields,
  };
}

export default FormBuilder;
