'use client';

import { yupResolver } from '@hookform/resolvers/yup';
import {
  type FieldValues,
  type Path,
  type Resolver,
  type UseFormProps,
  type UseFormReturn,
  useForm,
} from 'react-hook-form';
import type { ObjectSchema } from 'yup';

export const useFormWithValidation = <TFieldValues extends FieldValues = FieldValues>(
  schema: ObjectSchema<TFieldValues>,
  options?: Omit<UseFormProps<TFieldValues>, 'resolver'>,
) => {
  return useForm<TFieldValues>({
    resolver: yupResolver(schema) as Resolver<TFieldValues>,
    mode: 'onBlur',
    ...options,
  });
};

export const getErrorMessage = <TFieldValues extends FieldValues>(
  errors: UseFormReturn<TFieldValues>['formState']['errors'],
  fieldName: Path<TFieldValues>,
): string | undefined => {
  const keys = fieldName.split('.');
  let error: unknown = errors;

  for (const key of keys) {
    if (error && typeof error === 'object' && key in error) {
      error = (error as Record<string, unknown>)[key];
    } else {
      return undefined;
    }
  }

  return error && typeof error === 'object' && 'message' in error
    ? String(error.message)
    : undefined;
};

export const hasError = <TFieldValues extends FieldValues>(
  errors: UseFormReturn<TFieldValues>['formState']['errors'],
  fieldName: Path<TFieldValues>,
): boolean => {
  return !!getErrorMessage(errors, fieldName);
};

export const formatFormData = <T extends FieldValues>(data: T): FormData => {
  const formData = new FormData();

  Object.entries(data).forEach(([key, value]) => {
    if (value instanceof File) {
      formData.append(key, value);
    } else if (
      value &&
      typeof value === 'object' &&
      'length' in value &&
      typeof value.length === 'number'
    ) {
      Array.from(value as ArrayLike<File>).forEach((file) => {
        formData.append(key, file);
      });
    } else if (Array.isArray(value)) {
      value.forEach((item, index) => {
        if (typeof item === 'object' && item !== null) {
          formData.append(`${key}[${index}]`, JSON.stringify(item));
        } else {
          formData.append(`${key}[${index}]`, item);
        }
      });
    } else if (typeof value === 'object' && value !== null) {
      formData.append(key, JSON.stringify(value));
    } else if (value !== null && value !== undefined) {
      formData.append(key, String(value));
    }
  });

  return formData;
};

export const resetFormWithDefaults = <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
  defaultValues?: Partial<TFieldValues>,
): void => {
  form.reset(defaultValues as TFieldValues);
};

export const setFormErrors = <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
  errors: Record<string, string>,
): void => {
  Object.entries(errors).forEach(([field, message]) => {
    form.setError(field as Path<TFieldValues>, {
      type: 'manual',
      message,
    });
  });
};

export const clearFormErrors = <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
): void => {
  form.clearErrors();
};

export const getFormDataAsJson = <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
): string => {
  return JSON.stringify(form.getValues());
};

export const loadFormDataFromJson = <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
  jsonString: string,
): void => {
  try {
    const data = JSON.parse(jsonString);
    form.reset(data);
  } catch (_error) {}
};

export const validateField = async <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
  fieldName: Path<TFieldValues>,
): Promise<boolean> => {
  return form.trigger(fieldName);
};

export const validateFields = async <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
  fieldNames: Path<TFieldValues>[],
): Promise<boolean> => {
  return form.trigger(fieldNames);
};

export const getDirtyFields = <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
): Partial<TFieldValues> => {
  const dirtyFields: Partial<TFieldValues> = {};
  const allValues = form.getValues();
  const dirtyFieldsState = form.formState.dirtyFields;

  Object.keys(dirtyFieldsState).forEach((key) => {
    if (dirtyFieldsState[key as keyof typeof dirtyFieldsState]) {
      dirtyFields[key as keyof TFieldValues] = allValues[key as keyof TFieldValues];
    }
  });

  return dirtyFields;
};
