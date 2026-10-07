import { Box, LinearProgress, Step, StepButton, Stepper, Typography } from '@mui/material';
import { useEffect, useId, useRef } from 'react';

/** Shared, responsive navigation. Forward movement is controlled by the form's validation. */
export function WizardProgress({
  titles,
  activeStep,
  onBackTo,
  disabled = false,
}: {
  titles: string[];
  activeStep: number;
  onBackTo: (step: number) => void;
  disabled?: boolean;
}) {
  const heading = useRef<HTMLHeadingElement>(null);
  const previous = useRef(activeStep);
  const id = useId();
  useEffect(() => {
    if (previous.current !== activeStep) heading.current?.focus();
    previous.current = activeStep;
  }, [activeStep]);
  return (
    <Box sx={{ mb: 3 }}>
      <Stepper
        activeStep={activeStep}
        sx={{
          display: { xs: 'none', md: titles.length > 1 && titles.length <= 5 ? 'flex' : 'none' },
          mb: 3,
        }}
      >
        {titles.map((title, index) => (
          <Step key={`${index}-${title}`} completed={index < activeStep}>
            <StepButton
              type="button"
              disabled={disabled || index > activeStep}
              onClick={() => onBackTo(index)}
              aria-current={index === activeStep ? 'step' : undefined}
            >
              {title}
            </StepButton>
          </Step>
        ))}
      </Stepper>
      <Typography variant="overline" color="text.secondary">
        Step {activeStep + 1} of {titles.length}
      </Typography>
      <Typography
        id={id}
        component="h2"
        variant="h6"
        ref={heading}
        tabIndex={-1}
        sx={{ outline: 'none' }}
      >
        {titles[activeStep]}
      </Typography>
      <LinearProgress
        aria-label="Form progress"
        variant="determinate"
        value={((activeStep + 1) / titles.length) * 100}
        sx={{ mt: 1.5, height: 4, borderRadius: 2 }}
      />
    </Box>
  );
}
