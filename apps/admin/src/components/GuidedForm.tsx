import { WizardProgress } from '@gaming-cafe/ui';
import { Alert, Box, Button, Stack, Typography } from '@mui/material';
import {
  Children,
  isValidElement,
  type ReactElement,
  type ReactNode,
  useRef,
  useState,
} from 'react';

export interface GuidedStepProps {
  title: string;
  description?: string;
  children: ReactNode;
  validate?: () => string | undefined;
}
export function GuidedStep({ children }: GuidedStepProps) {
  return <>{children}</>;
}

/** Keeps every step mounted, including uploads/search pickers, while preserving local drafts. */
export function GuidedForm({
  children,
  actions,
  busy = false,
  onCancel,
  reviewDescription,
  review,
  enabled = true,
}: {
  children: ReactNode;
  actions: ReactNode;
  busy?: boolean;
  onCancel?: () => void;
  reviewDescription?: string;
  review?: ReactNode;
  enabled?: boolean;
}) {
  const steps = Children.toArray(children).filter(
    (child): child is ReactElement<GuidedStepProps> =>
      isValidElement<GuidedStepProps>(child) && child.type === GuidedStep,
  );
  const [active, setActive] = useState(0);
  const [error, setError] = useState('');
  const root = useRef<HTMLDivElement>(null);
  const reviewing = active === steps.length;
  function validate(index: number): boolean {
    const message = steps[index]?.props.validate?.();
    const panel = root.current?.querySelector(`[data-guided-step="${index}"]`);
    const invalid = Array.from(
      panel?.querySelectorAll<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>(
        'input,select,textarea',
      ) ?? [],
    ).find((input) => !input.disabled && !input.checkValidity());
    if (message || invalid) {
      setError(message || invalid?.validationMessage || 'Complete the required fields.');
      setActive(index);
      if (invalid)
        setTimeout(() => {
          invalid.focus();
          invalid.reportValidity();
        }, 0);
      return false;
    }
    setError('');
    return true;
  }
  function advance() {
    if (!busy && validate(active)) setActive(active + 1);
  }
  if (!enabled)
    return (
      <Stack spacing={3}>
        {children}
        {actions}
      </Stack>
    );
  return (
    <Box
      ref={root}
      sx={{ minWidth: 0, width: '100%' }}
      onKeyDown={(event) => {
        if (
          event.key === 'Enter' &&
          !reviewing &&
          event.target instanceof HTMLInputElement &&
          !['checkbox', 'radio', 'file', 'button', 'submit'].includes(event.target.type) &&
          event.target.getAttribute('role') !== 'combobox'
        ) {
          event.preventDefault();
          event.stopPropagation();
          advance();
        }
      }}
    >
      <WizardProgress
        titles={[...steps.map((step) => step.props.title), 'Review & confirm']}
        activeStep={active}
        disabled={busy}
        onBackTo={(index) => {
          setActive(index);
          setError('');
        }}
      />
      {error && (
        <Alert severity="error" sx={{ mb: 2 }}>
          {error}
        </Alert>
      )}
      {reviewing && (
        <Alert severity="info" sx={{ mb: 3 }}>
          {reviewDescription ||
            'Review your details below. You can still make corrections before confirming this record.'}
        </Alert>
      )}
      {reviewing && review}
      <Stack
        component="fieldset"
        disabled={busy}
        spacing={3}
        sx={{ m: 0, p: 0, border: 0, minWidth: 0, pointerEvents: busy ? 'none' : undefined }}
      >
        {steps.map((step, index) => (
          <Box
            key={step.key ?? index}
            data-guided-step={index}
            hidden={reviewing ? Boolean(review) : active !== index}
            sx={{ '&[hidden]': { display: 'none' } }}
          >
            {reviewing && (
              <Stack
                direction="row"
                justifyContent="space-between"
                alignItems="center"
                sx={{ mb: 2 }}
              >
                <Typography component="h3" variant="subtitle1" fontWeight={600}>
                  {step.props.title}
                </Typography>
                <Button type="button" disabled={busy} onClick={() => setActive(index)}>
                  Edit step
                </Button>
              </Stack>
            )}
            {step.props.description && (
              <Typography color="text.secondary" variant="body2" sx={{ mb: 2 }}>
                {step.props.description}
              </Typography>
            )}
            <Stack spacing={2.5}>{step.props.children}</Stack>
          </Box>
        ))}
      </Stack>
      <Stack
        direction={{ xs: 'column-reverse', sm: 'row' }}
        spacing={1.5}
        justifyContent="flex-end"
        sx={{ mt: 3, pt: 2, borderTop: 1, borderColor: 'divider' }}
      >
        {!reviewing && onCancel && (
          <Button
            data-wizard-cancel
            type="button"
            color="inherit"
            disabled={busy}
            onClick={onCancel}
          >
            Cancel
          </Button>
        )}
        {active > 0 && (
          <Button
            data-wizard-cancel
            type="button"
            variant="outlined"
            disabled={busy}
            onClick={() => {
              setActive(active - 1);
              setError('');
            }}
          >
            Back
          </Button>
        )}
        {!reviewing ? (
          <Button type="button" variant="contained" disabled={busy} onClick={advance}>
            {active === steps.length - 1 ? 'Review details' : 'Continue'}
          </Button>
        ) : (
          <Box
            component="fieldset"
            disabled={busy}
            sx={{
              border: 0,
              p: 0,
              m: 0,
              minWidth: 0,
              display: 'flex',
              gap: 1.5,
              flexWrap: 'wrap',
              '& > .MuiStack-root': { flexWrap: 'wrap', rowGap: 1 },
            }}
            onClickCapture={(event) => {
              // Existing mutation handlers remain authoritative; validate every step before they run.
              const button = (event.target as Element).closest('button');
              if (!button) return;
              if (busy) {
                event.preventDefault();
                event.stopPropagation();
                return;
              }
              if (button.hasAttribute('data-wizard-cancel')) return;
              if (steps.some((_, index) => !validate(index))) {
                event.preventDefault();
                event.stopPropagation();
              }
            }}
          >
            {actions}
          </Box>
        )}
      </Stack>
    </Box>
  );
}
