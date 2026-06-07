import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreateUnitFormData,
  createUnitDefaultValues,
  createUnitSchema,
  unitTypeOptions,
} from '../../../../src/containers/units/schemas/unit-schema';
import { addUnit } from '../../../services/units/add';
import type { UnitType } from '../../../services/units/list';

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
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  const handleSubmit = async (data: CreateUnitFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);
    if (!data.name || !data.abbreviation) {
      setError('Name and abbreviation are required');
      setLoading(false);
      return;
    }
    try {
      await addUnit({
        name: data.name,
        abbreviation: data.abbreviation,
        type: data.type as UnitType,
        description: data.description || '',
        isActive: data.isActive,
      });

      setSuccess('Unit created successfully!');

      // Navigate back to units list after a short delay
      setTimeout(() => {
        navigate('/units');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create unit');
    } finally {
      setLoading(false);
    }
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
        error={error}
        success={success}
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
