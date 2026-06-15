export const otpInputProps = {
  inputMode: 'numeric' as const,
  autoComplete: 'one-time-code' as const,
};

export const phoneInputProps = {
  inputMode: 'numeric' as const,
  autoComplete: 'tel' as const,
};

export const integerInputProps = {
  type: 'text' as const,
  inputMode: 'numeric' as const,
};

export const usernameInputProps = {
  autoComplete: 'username' as const,
};

export const currentPasswordInputProps = {
  autoComplete: 'current-password' as const,
};
