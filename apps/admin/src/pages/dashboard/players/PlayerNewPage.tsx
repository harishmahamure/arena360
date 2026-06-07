import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import { useMemo, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  adminCreateRoleOptions,
  type CreatePlayerFormData,
  createPlayerDefaultValues,
  createPlayerSchema,
} from '../../../../src/containers/players/schemas/player-schema';
import { usePermissions } from '../../../hooks/usePermissions';
import { addPlayer } from '../../../services/players/add';

const basePlayerFormFields: FieldConfig<CreatePlayerFormData>[] = [
  {
    name: 'username',
    label: 'Username',
    type: 'text',
    placeholder: 'e.g., johndoe',
    required: true,
    gridCols: 6,
    helperText: '3-50 characters, must be unique',
  },
  {
    name: 'phoneNumber',
    label: 'Phone Number',
    type: 'text',
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
  const { isAdmin } = usePermissions();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  const playerFormFields = useMemo<FieldConfig<CreatePlayerFormData>[]>(() => {
    if (!isAdmin) {
      return basePlayerFormFields;
    }

    return [
      ...basePlayerFormFields,
      {
        name: 'role',
        label: 'Role',
        type: 'select',
        gridCols: 6,
        options: adminCreateRoleOptions,
        helperText: 'Only admins can create staff accounts',
      },
    ];
  }, [isAdmin]);

  const handleSubmit = async (data: CreatePlayerFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.username || !data.password || !data.phoneNumber) {
      setError('Username, phone number, and password are required');
      setLoading(false);
      return;
    }

    try {
      await addPlayer({
        username: data.username,
        password: data.password,
        phoneNumber: data.phoneNumber,
        firstName: data.firstName || undefined,
        lastName: data.lastName || undefined,
        role: isAdmin ? data.role : 'player',
      });

      setSuccess('Player created successfully!');

      setTimeout(() => {
        navigate('/players');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create player');
    } finally {
      setLoading(false);
    }
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
        fields={playerFormFields}
        schema={createPlayerSchema}
        defaultValues={createPlayerDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={loading}
        error={error}
        success={success}
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
