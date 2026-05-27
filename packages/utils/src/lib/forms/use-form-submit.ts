'use client';

import { useCallback, useState } from 'react';
import type { FieldValues, Path, UseFormReturn } from 'react-hook-form';
import { isApiError } from '../../http';

export interface UseFormSubmitOptions<TFieldValues extends FieldValues> {
  onSuccess?: (data: unknown) => void;
  onError?: (error: unknown) => void;
  resetOnSuccess?: boolean;
  showSuccessMessage?: boolean;
  showErrorMessage?: boolean;
  transformData?: (data: TFieldValues) => unknown;
}

export interface UseFormSubmitReturn<TFieldValues extends FieldValues> {
  isSubmitting: boolean;
  submitError: string | null;
  handleSubmit: (
    submitFn: (data: TFieldValues) => Promise<unknown>,
  ) => (data: TFieldValues) => Promise<void>;
  reset: () => void;
}

export const useFormSubmit = <TFieldValues extends FieldValues>(
  form: UseFormReturn<TFieldValues>,
  options?: UseFormSubmitOptions<TFieldValues>,
): UseFormSubmitReturn<TFieldValues> => {
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [submitError, setSubmitError] = useState<string | null>(null);

  const handleSubmit = useCallback(
    (submitFn: (data: TFieldValues) => Promise<unknown>) => {
      return async (data: TFieldValues) => {
        setIsSubmitting(true);
        setSubmitError(null);

        try {
          // Transform data if transformer provided
          const transformedData = options?.transformData ? options.transformData(data) : data;

          // Submit the form
          const response = await submitFn(transformedData as TFieldValues);

          // Handle success
          if (options?.resetOnSuccess) {
            form.reset();
          }

          if (options?.onSuccess) {
            options.onSuccess(response);
          }

          if (options?.showSuccessMessage) {
            // You can integrate with a toast notification library here
          }
        } catch (error: unknown) {
          const errorMessage = isApiError(error)
            ? error.message
            : error instanceof Error
              ? error.message
              : typeof error === 'object' && error !== null && 'message' in error
                ? String(error.message)
                : 'An error occurred during form submission';

          setSubmitError(errorMessage);

          if (options?.onError) {
            options.onError(error);
          }

          if (options?.showErrorMessage) {
            // You can integrate with a toast notification library here
          }

          // Set server errors on form fields if available
          if (
            typeof error === 'object' &&
            error !== null &&
            'errors' in error &&
            typeof error.errors === 'object' &&
            error.errors !== null
          ) {
            Object.entries(error.errors).forEach(([field, messages]) => {
              if (Array.isArray(messages) && messages.length > 0) {
                form.setError(field as Path<TFieldValues>, {
                  type: 'server',
                  message: messages[0],
                });
              }
            });
          }
        } finally {
          setIsSubmitting(false);
        }
      };
    },
    [form, options],
  );

  const reset = useCallback(() => {
    setSubmitError(null);
    setIsSubmitting(false);
  }, []);

  return {
    isSubmitting,
    submitError,
    handleSubmit,
    reset,
  };
};
