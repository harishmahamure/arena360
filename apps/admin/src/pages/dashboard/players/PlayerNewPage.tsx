import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import { USERNAME_HELPER_TEXT, useAsyncAction } from '@gaming-cafe/utils';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type CreatePlayerFormData,
  createPlayerDefaultValues,
  createPlayerSchema,
} from '../../../../src/containers/players/schemas/player-schema';
import { addPlayer } from '../../../services/players/add';

const basePlayerFormFields: FieldConfig<CreatePlayerFormData>[] = [
  {
    name: 'username',
    label: 'Username',
    type: 'username',
    placeholder: 'e.g., johndoe',
    required: true,
    gridCols: 6,
    helperText: USERNAME_HELPER_TEXT,
  },
  {
    name: 'phoneNumber',
    label: 'Phone Number',
    type: 'phone',
    placeholder: 'e.g., 9876543210',
    required: true,
    gridCols: 6,
    helperText: 'Required, minimum 10 digits',
  },
  {
    name: 'firstName',
    label: 'First Name',
    type: 'text',
    placeholder: 'e.g., John',
    gridCols: 6,
    helperText: 'Optional (max 50 characters)',
  },
  {
    name: 'lastName',
    label: 'Last Name',
    type: 'text',
    placeholder: 'e.g., Doe',
    gridCols: 6,
    helperText: 'Optional (max 50 characters)',
  },
  {
    name: 'password',
    label: 'Password',
    type: 'password',
    placeholder: 'Enter password',
    required: true,
    gridCols: 6,
    helperText: 'Minimum 8 characters',
  },
  {
    name: 'confirmPassword',
    label: 'Confirm Password',
    type: 'password',
    placeholder: 'Confirm password',
    required: true,
    gridCols: 6,
    helperText: 'Must match password',
  },
];

export default function AddNewPlayerPage() {
  const navigate = useNavigate();
  const { loading, succeeded, failed, errorMessage, run } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });
  const [error, setError] = useState<string | undefined>();

  const playerFormFields = basePlayerFormFields;

  const handleSubmit = async (data: CreatePlayerFormData) => {
    setError(undefined);

    if (!data.username || !data.password || !data.phoneNumber) {
      setError('Username, phone number, and password are required');
      return;
    }

    void run(async () => {
      await addPlayer({
        username: data.username,
        password: data.password,
        phoneNumber: data.phoneNumber,
        firstName: data.firstName || undefined,
        lastName: data.lastName || undefined,
        role: 'player',
      });

      setTimeout(() => {
        navigate('/players');
      }, 1500);
    });
  };

  const handleCancel = () => {
    navigate('/players');
  };

  return (
    <FormPage
      title="Add New Player"
      description="Fill in the details below to create a new player account"
      backTo="/players"
      backLabel="Back to players"
    >
      <FormBuilder<CreatePlayerFormData>
        wizard
        wizardSteps={[
          { title: 'Player profile', fields: ['username', 'phoneNumber', 'firstName', 'lastName'] },
          { title: 'Access & security', fields: ['password', 'confirmPassword', 'role'] },
        ]}
        fields={playerFormFields}
        schema={createPlayerSchema}
        defaultValues={createPlayerDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={loading}
        submitSuccess={succeeded}
        submitSuccessLabel="Player created"
        submitError={failed}
        submitErrorLabel={errorMessage ?? 'Failed to create player'}
        error={error}
        showCancel
        showReset
        submitLabel="Create Player"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />
    </FormPage>
  );
}
