import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreatePlanFormData,
  createPlanDefaultValues,
  createPlanSchema,
  deviceSubTypeOptions,
  deviceTypeOptions,
  planTypeOptions,
} from '../../../../src/containers/plans/schemas/plan.schema';
import { addPlan, type CreatePlanPayload } from '../../../services/plans/add';
import type { PlanType } from '../../../services/plans/list';

export const planFormFields: FieldConfig<CreatePlanFormData>[] = [
  {
    name: 'name',
    label: 'Plan Name',
    type: 'text',
    placeholder: 'e.g., 2 Hour Gaming Pass',
    required: true,
    gridCols: 6,
    helperText: 'Enter the plan name (max 100 characters)',
  },
  {
    name: 'planType',
    label: 'Plan Type',
    type: 'select',
    required: true,
    gridCols: 6,
    options: planTypeOptions,
    helperText: 'Select the type of plan',
  },
  {
    name: 'description',
    label: 'Description',
    type: 'textarea',
    placeholder: 'Enter plan description...',
    fullWidth: true,
    rows: 3,
    helperText: 'Optional plan description',
  },
  {
    name: 'price',
    label: 'Price ($)',
    type: 'number',
    placeholder: '29.99',
    required: true,
    gridCols: 4,
    min: 0.01,
    helperText: 'Plan price (must be > 0)',
  },
  {
    name: 'validityDays',
    label: 'Validity Days',
    type: 'number',
    placeholder: '30',
    gridCols: 4,
    min: 1,
    helperText: 'Number of days plan remains valid',
  },
  {
    name: 'durationMinutes',
    label: 'Duration (Minutes)',
    type: 'number',
    placeholder: '120',
    gridCols: 4,
    min: 1,
    helperText: 'Required for time-based plans',
    /** @deprecated 'showWhen' is not a valid property for FieldConfig and has been removed */
    // showWhen: (values: CreatePlanFormData) =>
    //   values.planType === PlanTypeValues.TIME_BASED,
  },
  {
    name: 'timeCredits',
    label: 'Time Credits (Minutes)',
    type: 'number',
    placeholder: '120',
    gridCols: 4,
    min: 1,
    helperText: 'Available time credits (for time-based plans)',
  },
  {
    name: 'maxSessions',
    label: 'Max Sessions',
    type: 'number',
    placeholder: '5',
    gridCols: 4,
    min: 1,
    helperText: 'Maximum number of sessions allowed',
  },
  {
    name: 'perMinuteRate',
    label: 'Per Minute Rate',
    type: 'number',
    placeholder: '1.0',
    gridCols: 4,
    min: 0.01,
    helperText: 'Rate per minute (for hourly rental)',
  },
  {
    name: 'deviceType',
    label: 'Device Type',
    type: 'select',
    options: deviceTypeOptions,
    helperText: 'Select the type of device',
  },
  {
    name: 'deviceSubType',
    label: 'Device Sub Type',
    type: 'select',
    options: deviceSubTypeOptions,
    helperText: 'Select the sub type of device',
  },
  {
    name: 'timeWindowStart',
    label: 'Time Window Start (HH:MM:SS)',
    type: 'text',
    placeholder: '07:00:00',
    gridCols: 6,
    helperText: 'Start time for time window restrictions',
  },
  {
    name: 'timeWindowEnd',
    label: 'Time Window End (HH:MM:SS)',
    type: 'text',
    placeholder: '23:00:00',
    gridCols: 6,
    helperText: 'End time for time window restrictions',
  },
  {
    name: 'isActive',
    label: 'Active (Available for purchase)',
    type: 'switch',
    gridCols: 12,
    helperText: 'Toggle to make plan available for purchase',
  },
];

export default function AddNewPlanPage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  const handleSubmit = async (data: CreatePlanFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.name || !data.price || !data.planType) {
      setError('Name, price, and plan type are required');
      setLoading(false);
      return;
    }

    try {
      const payload: CreatePlanPayload = {
        name: data.name,
        description: data.description || '',
        price: data.price,
        planType: data.planType as PlanType,
        validityDays: data.validityDays || 30,
        perMinuteRate: data.perMinuteRate || 1.0,
        isActive: data.isActive ?? true,
      };

      // Add conditional fields based on plan type
      if (data.durationMinutes) payload.durationMinutes = data.durationMinutes;
      if (data.timeCredits) payload.timeCredits = data.timeCredits;
      if (data.maxSessions) payload.maxSessions = data.maxSessions;
      if (data.timeWindowStart) payload.timeWindowStart = data.timeWindowStart;
      if (data.timeWindowEnd) payload.timeWindowEnd = data.timeWindowEnd;

      await addPlan(payload);

      setSuccess('Plan created successfully!');

      // Navigate back to plans list after a short delay
      setTimeout(() => {
        navigate('/plans');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create plan');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/plans');
  };

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Add New Plan
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Fill in the details below to create a new gaming plan
        </Typography>
      </Box>

      <FormBuilder<CreatePlanFormData>
        fields={planFormFields}
        schema={createPlanSchema}
        defaultValues={createPlanDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={loading}
        error={error}
        success={success}
        showCancel
        showReset
        submitLabel="Create Plan"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
