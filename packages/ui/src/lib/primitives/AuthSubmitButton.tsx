export interface AuthSubmitButtonProps
  extends Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'className'> {
  busy?: boolean;
}

export function AuthSubmitButton({
  busy = false,
  disabled,
  children,
  ...buttonProps
}: AuthSubmitButtonProps) {
  return (
    <button className="gz-auth-submit" disabled={disabled || busy} {...buttonProps}>
      {children}
    </button>
  );
}
