import type { DeductionProfile, PlanTypeValue } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreatePlanFormData,
  createPlanDefaultValues,
  createPlanSchema,
  deviceSubTypeOptions,
  deviceTypeOptions,
  MONTH_OPTIONS,
  planTypeOptions,
  WEEKDAY_OPTIONS,
} from '../../../../src/containers/plans/schemas/plan.schema';
import { DeductionPreview } from '../../../containers/plans/DeductionPreview';
import { addPlan, type CreatePlanPayload } from '../../../services/plans/add';

function normalizeTime(value?: string): string | undefined {
  if (!value) return undefined;
  return value.length === 5 ? `${value}:00` : value;
}

function buildDeductionProfile(data: CreatePlanFormData): DeductionProfile | undefined {
  if (!data.dynamicDeductionEnabled) return undefined;
  return {
    peakWindowStart: normalizeTime(data.peakWindowStart) ?? '',
    peakWindowEnd: normalizeTime(data.peakWindowEnd) ?? '',
    peakRatio: data.peakRatio ?? 1.5,
    lowWindowStart: normalizeTime(data.lowWindowStart) ?? '',
    lowWindowEnd: normalizeTime(data.lowWindowEnd) ?? '',
    lowRatio: data.lowRatio ?? 0.8,
  };
}

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
    label: 'Price',
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
    placeholder: '7',
    gridCols: 4,
    min: 1,
    helperText: 'Number of days plan remains valid',
  },
  {
    name: 'timeCredits',
    label: 'Time Credits (Minutes)',
    type: 'number',
    placeholder: '300',
    gridCols: 4,
    min: 1,
    required: true,
    helperText: 'Available time credits in minutes',
  },
  {
    name: 'deviceType',
    label: 'Device Type',
    type: 'select',
    gridCols: 6,
    options: deviceTypeOptions,
    helperText: 'Select the type of device',
  },
  {
    name: 'deviceSubType',
    label: 'Device Sub Type',
    type: 'select',
    gridCols: 6,
    options: deviceSubTypeOptions,
    helperText: 'Select the sub type of device',
  },
  {
    name: 'timeWindowStart',
    label: 'Time Window Start (HH:MM:SS)',
    type: 'text',
    placeholder: '06:00:00',
    gridCols: 6,
    helperText: 'Required for Happy Hours plans',
  },
  {
    name: 'timeWindowEnd',
    label: 'Time Window End (HH:MM:SS)',
    type: 'text',
    placeholder: '11:59:00',
    gridCols: 6,
    helperText: 'Required for Happy Hours plans',
  },
  {
    name: 'allowedDays',
    label: 'Allowed Days',
    type: 'multiselect',
    gridCols: 6,
    options: WEEKDAY_OPTIONS,
    helperText: 'Restrict to specific days (leave empty for all days)',
  },
  {
    name: 'allowedMonths',
    label: 'Allowed Months',
    type: 'multiselect',
    gridCols: 6,
    options: MONTH_OPTIONS,
    helperText: 'Restrict to specific months (leave empty for all months)',
  },
  {
    name: 'isActive',
    label: 'Active (Available for purchase)',
    type: 'switch',
    gridCols: 12,
    helperText: 'Toggle to make plan available for purchase',
  },
  {
    name: 'dynamicDeductionEnabled',
    label: 'Dynamic deduction enabled',
    type: 'switch',
    gridCols: 12,
    helperText: 'Wallet minutes burn faster in peak hours and slower in low hours',
  },
  {
    name: 'peakWindowStart',
    label: 'Peak window start',
    type: 'text',
    placeholder: '18:00:00',
    gridCols: 3,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
  },
  {
    name: 'peakWindowEnd',
    label: 'Peak window end',
    type: 'text',
    placeholder: '23:00:00',
    gridCols: 3,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
  },
  {
    name: 'peakRatio',
    label: 'Peak ratio (>1)',
    type: 'number',
    gridCols: 3,
    min: 1.01,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
    helperText: 'Wallet minutes consumed per wall minute',
  },
  {
    name: 'lowWindowStart',
    label: 'Low window start',
    type: 'text',
    placeholder: '07:00:00',
    gridCols: 3,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
  },
  {
    name: 'lowWindowEnd',
    label: 'Low window end',
    type: 'text',
    placeholder: '11:00:00',
    gridCols: 3,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
  },
  {
    name: 'lowRatio',
    label: 'Low ratio (<1)',
    type: 'number',
    gridCols: 3,
    min: 0.01,
    max: 0.99,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
  },
  {
    name: 'deductionPreviewNote',
    label: 'Deduction calculator',
    type: 'custom',
    gridCols: 12,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
    render: ({ form }) => (
      <DeductionPreview
        timeCredits={Number(form.getValues('timeCredits')) || 0}
        dynamicDeductionEnabled={form.getValues('dynamicDeductionEnabled')}
        peakWindowStart={form.getValues('peakWindowStart')}
        peakWindowEnd={form.getValues('peakWindowEnd')}
        peakRatio={Number(form.getValues('peakRatio'))}
        lowWindowStart={form.getValues('lowWindowStart')}
        lowWindowEnd={form.getValues('lowWindowEnd')}
        lowRatio={Number(form.getValues('lowRatio'))}
      />
    ),
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
        planType: data.planType as PlanTypeValue,
        validityDays: data.validityDays || 7,
        timeCredits: data.timeCredits,
        isActive: data.isActive ?? true,
        deviceType: data.deviceType || undefined,
        deviceSubType: data.deviceSubType || undefined,
      };

      if (data.timeWindowStart) payload.timeWindowStart = data.timeWindowStart;
      if (data.timeWindowEnd) payload.timeWindowEnd = data.timeWindowEnd;
      if (data.allowedDays?.length) payload.allowedDays = data.allowedDays;
      if (data.allowedMonths?.length) payload.allowedMonths = data.allowedMonths;
      payload.dynamicDeductionEnabled = data.dynamicDeductionEnabled ?? false;
      const deductionProfile = buildDeductionProfile(data);
      if (deductionProfile) payload.deductionProfile = deductionProfile;

      await addPlan(payload);

      setSuccess('Plan created successfully!');
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
    <FormPage
      title="Add New Plan"
      description="Fill in the details below to create a new gaming plan"
      backTo="/plans"
      backLabel="Back to plans"
      breadcrumbs={[{ label: 'Plans', to: '/plans' }, { label: 'New plan' }]}
    >
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
    </FormPage>
  );
}
