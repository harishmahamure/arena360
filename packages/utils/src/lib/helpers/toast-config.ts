import type { ToastContainerProps, ToastOptions } from 'react-toastify';

export const DEFAULT_TOAST_OPTIONS: ToastOptions = {
  position: 'bottom-center',
  autoClose: 3000,
  hideProgressBar: false,
  closeOnClick: true,
  pauseOnHover: true,
  draggable: true,
};

export const TOAST_CONTAINER_PROPS: Pick<
  ToastContainerProps,
  | 'position'
  | 'autoClose'
  | 'hideProgressBar'
  | 'newestOnTop'
  | 'closeOnClick'
  | 'rtl'
  | 'pauseOnFocusLoss'
  | 'draggable'
  | 'pauseOnHover'
  | 'limit'
> = {
  position: 'bottom-center',
  autoClose: 3000,
  hideProgressBar: false,
  newestOnTop: false,
  closeOnClick: true,
  rtl: false,
  pauseOnFocusLoss: true,
  draggable: true,
  pauseOnHover: true,
  limit: 3,
};
