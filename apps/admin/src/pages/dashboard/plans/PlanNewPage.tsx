import type { DeductionProfile, PlanTypeValue } from '@gaming-cafe/contracts';
import { PlanType } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder, FormPage, type FormSection } from '@gaming-cafe/ui';
import { normalizeTimeOfDay, useAsyncAction } from '@gaming-cafe/utils';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreatePlanFormData,
  createPlanDefaultValues,
  createPlanSchema,
  deviceSubTypeOptions,
  deviceTypeOptions,
} from '../../../../src/containers/plans/schemas/plan.schema';
import { TimeOfDayField } from '../../../components/forms/TimeOfDayField';
import { DeductionPreview } from '../../../containers/plans/DeductionPreview';
import { addPlan, type CreatePlanPayload } from '../../../services/plans/add';

function buildDeductionProfile(data: CreatePlanFormData): DeductionProfile | undefined {
  if (!data.dynamicDeductionEnabled) return undefined;
  return {
    peakWindowStart: normalizeTimeOfDay(data.peakWindowStart) ?? '',
    peakWindowEnd: normalizeTimeOfDay(data.peakWindowEnd) ?? '',
    peakRatio: data.peakRatio ?? 1.5,
    lowWindowStart: normalizeTimeOfDay(data.lowWindowStart) ?? '',
    lowWindowEnd: normalizeTimeOfDay(data.lowWindowEnd) ?? '',
    lowRatio: data.lowRatio ?? 0.8,
  };
}

function timeOfDayField(
  name: keyof CreatePlanFormData,
  label: string,
  gridCols = 4,
): FieldConfig<CreatePlanFormData> {
  return {
    name,
    label,
    type: 'custom',
    gridCols,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
    render: ({ field, fieldState }) => (
      <TimeOfDayField
        value={typeof field.value === 'string' ? field.value : undefined}
        onChange={field.onChange}
        onBlur={field.onBlur}
        error={Boolean(fieldState.error)}
        helperText={fieldState.error?.message}
      />
    ),
  };
}

const dynamicDeductionFields: FieldConfig<CreatePlanFormData>[] = [
  {
    name: 'dynamicDeductionEnabled',
    label: 'Dynamic deduction enabled',
    type: 'switch',
    gridCols: 12,
    helperText: 'Wallet minutes burn faster in peak hours and slower in low hours',
  },
  timeOfDayField('peakWindowStart', 'Peak window start'),
  timeOfDayField('peakWindowEnd', 'Peak window end'),
  {
    name: 'peakRatio',
    label: 'Peak ratio (>1)',
    type: 'number',
    gridCols: 4,
    min: 1.01,
    visible: (values) => Boolean(values.dynamicDeductionEnabled),
    helperText: 'Wallet minutes consumed per wall minute',
  },
  timeOfDayField('lowWindowStart', 'Low window start'),
  timeOfDayField('lowWindowEnd', 'Low window end'),
  {
    name: 'lowRatio',
    label: 'Low ratio (<1)',
    type: 'number',
    gridCols: 4,
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

export const planFormSections: FormSection<CreatePlanFormData>[] = [
  {
    title: 'Plan details',
    fields: [
      { name: 'planType', label: 'Plan Type', type: 'hidden' },
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
    ],
  },
  {
    title: 'Device targeting',
    fields: [
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
    ],
  },
  {
    title: 'Availability',
    fields: [
      {
        name: 'isActive',
        label: 'Active (Available for purchase)',
        type: 'switch',
        gridCols: 12,
        helperText: 'Toggle to make plan available for purchase',
      },
    ],
  },
  {
    title: 'Dynamic deduction',
    description: 'Optional peak/low hour wallet burn rates',
    fields: dynamicDeductionFields,
    showDivider: false,
  },
];

/** @deprecated Use planFormSections — kept for imports that expect flat fields */
export const planFormFields: FieldConfig<CreatePlanFormData>[] = planFormSections.flatMap(
  (section) => section.fields,
);

export default function AddNewPlanPage() {
  const navigate = useNavigate();
  const { loading, succeeded, failed, errorMessage, run } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });
  const [error, setError] = useState<string | undefined>();

  const handleSubmit = async (data: CreatePlanFormData) => {
    setError(undefined);

    if (!data.name || !data.price) {
      setError('Name and price are required');
      return;
    }

    const { name, price } = data;

    void run(async () => {
      const payload: CreatePlanPayload = {
        name,
        description: data.description || '',
        price,
        planType: PlanType.TIME_BASED as PlanTypeValue,
        validityDays: data.validityDays || 7,
        timeCredits: data.timeCredits,
        isActive: data.isActive ?? true,
        deviceType: data.deviceType || undefined,
        deviceSubType: data.deviceSubType || undefined,
      };

      payload.dynamicDeductionEnabled = data.dynamicDeductionEnabled ?? false;
      const deductionProfile = buildDeductionProfile(data);
      if (deductionProfile) payload.deductionProfile = deductionProfile;

      await addPlan(payload);

      setTimeout(() => {
        navigate('/plans');
      }, 1500);
    });
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
        sections={planFormSections}
        schema={createPlanSchema}
        defaultValues={createPlanDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={loading}
        submitSuccess={succeeded}
        submitSuccessLabel="Plan created"
        submitError={failed}
        submitErrorLabel={errorMessage ?? 'Failed to create plan'}
        error={error}
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
