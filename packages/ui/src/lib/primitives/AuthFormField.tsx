import { useId, useState } from 'react';

export interface AuthFormFieldProps
  extends Omit<React.InputHTMLAttributes<HTMLInputElement>, 'className' | 'id'> {
  label: string;
  id?: string;
  inputClassName?: string;
}

export function AuthFormField({
  label,
  id,
  type = 'text',
  inputClassName,
  ...inputProps
}: AuthFormFieldProps) {
  const generatedId = useId();
  const inputId = id ?? generatedId;
  const [showPassword, setShowPassword] = useState(false);
  const isPassword = type === 'password';
  const resolvedType = isPassword && showPassword ? 'text' : type;
  const input = (
    <input
      id={inputId}
      type={resolvedType}
      className={['gz-auth-input', inputClassName].filter(Boolean).join(' ')}
      {...inputProps}
    />
  );

  return (
    <div className="gz-auth-field">
      <label className="gz-auth-label" htmlFor={inputId}>
        {label}
      </label>
      {isPassword ? (
        <div className="gz-auth-input-wrapper">
          {input}
          <button
            type="button"
            className="gz-auth-input-toggle"
            aria-label={showPassword ? 'Hide password' : 'Show password'}
            disabled={inputProps.disabled}
            onClick={() => setShowPassword((value) => !value)}
          >
            {showPassword ? 'Hide' : 'Show'}
          </button>
        </div>
      ) : (
        input
      )}
    </div>
  );
}
