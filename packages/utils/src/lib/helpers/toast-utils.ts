import { type ToastOptions, toast } from 'react-toastify';
import { DEFAULT_TOAST_OPTIONS } from './toast-config';

export interface SessionWarningOptions {
  sessionId?: string;
  tag?: string;
  autoClose?: number;
}

export const toastUtils = {
  success: (message: string, options?: ToastOptions) => {
    toast.success(message, { ...DEFAULT_TOAST_OPTIONS, ...options });
  },

  error: (message: string, options?: ToastOptions) => {
    toast.error(message, { ...DEFAULT_TOAST_OPTIONS, ...options });
  },

  info: (message: string, options?: ToastOptions) => {
    toast.info(message, { ...DEFAULT_TOAST_OPTIONS, ...options });
  },

  warning: (message: string, options?: ToastOptions) => {
    toast.warning(message, { ...DEFAULT_TOAST_OPTIONS, ...options });
  },

  sessionWarning: (message: string, options?: SessionWarningOptions) => {
    const { sessionId, tag, autoClose = 10000 } = options ?? {};
    toast.warning(message, {
      ...DEFAULT_TOAST_OPTIONS,
      autoClose,
      toastId: tag,
      className: 'gc-toast--session-warning',
      onClick: sessionId
        ? () => {
            window.location.assign(`/sessions/${sessionId}`);
          }
        : undefined,
    });
  },

  default: (message: string, options?: ToastOptions) => {
    toast(message, { ...DEFAULT_TOAST_OPTIONS, ...options });
  },

  promise: <T>(
    promise: Promise<T>,
    messages: {
      pending: string;
      success: string;
      error: string;
    },
    options?: ToastOptions,
  ) => {
    return toast.promise(promise, messages, { ...DEFAULT_TOAST_OPTIONS, ...options });
  },

  dismiss: (toastId?: string | number) => {
    toast.dismiss(toastId);
  },

  dismissAll: () => {
    toast.dismiss();
  },
};
