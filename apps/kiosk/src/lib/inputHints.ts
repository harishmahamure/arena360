export {
  currentPasswordInputProps,
  integerInputProps,
  otpInputProps,
  phoneInputProps,
  usernameInputProps,
} from '@gaming-cafe/ui';

/** Player login fields — suppress browser/password-manager autofill on shared kiosks. */
export const playerLoginUsernameInputProps = {
  autoComplete: 'off',
  autoCorrect: 'off',
  autoCapitalize: 'off',
  spellCheck: false,
  name: 'arena360-kiosk-player-username',
} as const;

export const playerLoginPasswordInputProps = {
  autoComplete: 'new-password',
  autoCorrect: 'off',
  spellCheck: false,
  name: 'arena360-kiosk-player-password',
} as const;
