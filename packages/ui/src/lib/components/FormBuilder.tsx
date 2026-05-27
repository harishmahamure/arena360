'use client';

import { yupResolver } from '@hookform/resolvers/yup';
import {
  Alert,
  Box,
  Collapse,
  Divider,
  GridLegacy as Grid,
  Stack,
  Typography,
} from '@mui/material';
import { useCallback, useEffect, useMemo } from 'react';
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
import FileUpload from './forms/FileUpload';
import FormButton from './forms/FormButton';
import FormCheckbox from './forms/FormCheckbox';
import FormContainer from './forms/FormContainer';
import FormRadioGroup, { type FormRadioOption } from './forms/FormRadioGroup';
import FormSelect, { type FormSelectOption } from './forms/FormSelect';
import FormSwitch from './forms/FormSwitch';
import FormTextField from './forms/FormTextField';
import PasswordField from './forms/PasswordField';
import { RHFSearchOnEnterAutocomplete, type SearchOption } from './forms/SearchInput';

export type FieldType =
  | 'text'
  | 'email'
  | 'number'
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
  | 'search';

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

  /** Helper text for the field */
  formHelperText?: string;
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

  const commonProps = {
    fullWidth: true,
    error: !!errorMessage,
    helperText: errorMessage || helperText,
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
                  {...commonProps}
                  label={label + (required ? ' *' : '')}
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
                  helperText={formHelperText}
                />
              );

            case 'number':
              return (
                <FormTextField
                  {...field}
                  {...commonProps}
                  label={''}
                  type="number"
                  autoComplete="one-time-code"
                  inputProps={{ min, max, autoComplete: 'one-time-code' }}
                  InputLabelProps={{ shrink: true }}
                  value={field.value ?? ''}
                  onChange={(e) => {
                    const value = e.target.value;
                    field.onChange(value === '' ? '' : Number(value));
                  }}
                />
              );

            case 'textarea':
              return (
                <FormTextField
                  {...field}
                  {...commonProps}
                  label={''}
                  multiline
                  rows={rows}
                  value={field.value ?? ''}
                  autoComplete="one-time-code"
                  inputProps={{ autoComplete: 'one-time-code' }}
                  helperText={formHelperText}
                />
              );

            case 'password':
              return (
                <PasswordField
                  {...field}
                  {...commonProps}
                  label={''}
                  value={field.value ?? ''}
                  autoComplete="new-password"
                  inputProps={{
                    autoComplete: 'off',
                  }}
                  InputLabelProps={{ shrink: true }}
                  helperText={helperText || formHelperText}
                />
              );

            case 'select':
              return (
                <FormSelect
                  {...commonProps}
                  label={''}
                  options={options as FormSelectOption[]}
                  value={field.value ?? ''}
                  onChange={(e) => field.onChange(e.target.value)}
                  onBlur={field.onBlur}
                  name={field.name}
                  helperText={formHelperText}
                />
              );

            case 'checkbox':
              return (
                <FormCheckbox
                  {...commonProps}
                  label={''}
                  checked={!!field.value}
                  onChange={(e) => field.onChange(e.target.checked)}
                  onBlur={field.onBlur}
                  name={field.name}
                  inputRef={field.ref}
                />
              );

            case 'switch':
              return (
                <FormSwitch
                  {...commonProps}
                  label={''}
                  checked={!!field.value}
                  onChange={(e) => field.onChange(e.target.checked)}
                  onBlur={field.onBlur}
                  name={field.name}
                  inputRef={field.ref}
                />
              );

            case 'radio':
              return (
                <FormRadioGroup
                  {...commonProps}
                  label={''}
                  options={options as FormRadioOption[]}
                  value={field.value ?? ''}
                  onChange={(e) => field.onChange(e.target.value)}
                  onBlur={field.onBlur}
                  name={field.name}
                />
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
                  control={control as Control<any>}
                  label={''}
                  placeholder={placeholder}
                  onSearch={onSearch as (query: string) => Promise<SearchOption[]>}
                  disabled={isDisabled}
                  queryKey={searchQueryKey}
                  helperText={formHelperText}
                  multiple={multiple}
                  onSearchComplete={onSearchComplete}
                />
              );

            default:
              return (
                <FormTextField
                  {...field}
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
  onSubmit,
  onCancel,
  onReset,
  onChange,
  submitLabel,
  cancelLabel = 'Cancel',
  resetLabel = 'Reset',
  loading = false,
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
    if (loading || isSubmitting) return true;
    if (isEditMode && requireDirty && !isDirty) return true;
    return false;
  }, [isViewMode, loading, isSubmitting, isEditMode, requireDirty, isDirty]);

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
    onReset?.();
  }, [reset, defaultValues, onReset]);

  // Watch for changes and notify
  useEffect(() => {
    if (onChange) {
      const dirty = dirtyFields as Partial<Record<keyof T, boolean>>;
      onChange(watchedValues as T, dirty);
    }
  }, [watchedValues, dirtyFields, onChange]);

  // Combine fields from both flat and sections
  const allFields = useMemo(() => {
    if (sections.length > 0) {
      return sections.flatMap((section) => section.fields);
    }
    return fields;
  }, [fields, sections]);

  // Render fields with grid
  const renderFields = (fieldsToRender: FieldConfig<T>[]) => (
    <Grid container spacing={spacing}>
      {fieldsToRender.map((fieldConfig) => {
        const gridCols = fieldConfig.fullWidth ? 12 : fieldConfig.gridCols || 6;

        return (
          <Grid item xs={12} sm={gridCols} key={fieldConfig.name} component="div">
            <Typography variant="body2" color="text.secondary" sx={{ pb: 1, px: 0.2 }}>
              {fieldConfig.label}
              {fieldConfig.required && <span style={{ color: 'red' }}>*</span>}
            </Typography>
            <FieldRenderer
              config={fieldConfig}
              control={control}
              errors={errors}
              mode={mode}
              watch={watch}
              form={form}
              onSearchComplete={onSearchComplete}
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
        <Box key={index}>
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
      onSubmit={handleSubmit(handleFormSubmit)}
      style={{
        width: '100%',
        maxWidth: '100%',
        padding: 24,
        backgroundColor: 'background.paper',
        borderRadius: 2,
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

      <Box sx={{ mb: 3 }}>{sections.length > 0 ? renderSections() : renderFields(allFields)}</Box>

      {isEditMode && isDirty && (
        <Box sx={{ mb: 2 }}>
          <Typography variant="caption" color="warning.main">
            Unsaved changes:{' '}
            {Object.keys(dirtyFields)
              .filter((key) => (dirtyFields as Record<string, boolean>)[key])
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

          {actions}

          <FormButton
            type="submit"
            variant="contained"
            color="primary"
            loading={loading || isSubmitting}
            disabled={isSubmitDisabled}
          >
            {computedSubmitLabel}
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
