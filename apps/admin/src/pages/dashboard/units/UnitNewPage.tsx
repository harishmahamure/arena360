import type { UnitTypeValue } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreateUnitFormData,
  createUnitDefaultValues,
  createUnitSchema,
  unitTypeOptions,
} from '../../../../src/containers/units/schemas/unit-schema';
import { addUnit } from '../../../services/units/add';

export const unitFormFields: FieldConfig<CreateUnitFormData>[] = [
  {
    name: 'name',
    label: 'Unit Name',
    type: 'text',
    placeholder: 'e.g., Box',
    required: true,
    gridCols: 6,
    helperText: 'Enter the unit name (max 100 characters)',
  },
  {
    name: 'abbreviation',
    label: 'Abbreviation',
    type: 'text',
    required: true,
    placeholder: 'e.g., box',
    gridCols: 6,
    helperText: 'Unit abbreviation (max 20 characters)',
  },
  {
    name: 'type',
    label: 'Unit Type',
    type: 'select',
    required: false,
    gridCols: 12,
    options: unitTypeOptions,
    helperText: 'Select the type of unit',
  },
  {
    name: 'description',
    label: 'Description',
    type: 'textarea',
    placeholder: 'Enter unit description...',
    fullWidth: true,
    rows: 3,
    helperText: 'Optional unit description',
  },
  {
    name: 'isActive',
    label: 'Active',
    type: 'switch',
    gridCols: 12,
    helperText: 'Toggle to make unit active',
  },
];

export default function AddNewUnitPage() {
  const navigate = useNavigate();
  const { loading, succeeded, failed, errorMessage, run } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });
  const [error, setError] = useState<string | undefined>();

  const handleSubmit = async (data: CreateUnitFormData) => {
    setError(undefined);
    if (!data.name || !data.abbreviation) {
      setError('Name and abbreviation are required');
      return;
    }
    const { name, abbreviation } = data;
    void run(async () => {
      await addUnit({
        name,
        abbreviation,
        type: data.type as UnitTypeValue,
        description: data.description || '',
        isActive: data.isActive,
      });

      setTimeout(() => {
        navigate('/units');
      }, 1500);
    });
  };

  const handleCancel = () => {
    navigate('/units');
  };

  return (
    <FormPage
      title="Add New Unit"
      description="Fill in the details below to create a new measurement unit"
      backTo="/units"
      backLabel="Back to units"
      breadcrumbs={[{ label: 'Units', to: '/units' }, { label: 'New unit' }]}
    >
      <FormBuilder<CreateUnitFormData>
        fields={unitFormFields}
        schema={createUnitSchema}
        defaultValues={createUnitDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={loading}
        submitSuccess={succeeded}
        submitSuccessLabel="Unit created"
        submitError={failed}
        submitErrorLabel={errorMessage ?? 'Failed to create unit'}
        error={error}
        showCancel
        showReset
        submitLabel="Create Unit"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />
    </FormPage>
  );
}
