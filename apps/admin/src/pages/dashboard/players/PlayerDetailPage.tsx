import type { UserRole } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  adminCreateRoleOptions,
  type UpdatePlayerFormData,
  updatePlayerSchema,
  userRoleOptions,
} from '../../../containers/players/schemas/player-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getPlayerById } from '../../../services/players/getById';
import { updatePlayer } from '../../../services/players/update';

export default function EditPlayerPage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const { can, isAdmin } = usePermissions();
  const canWrite = can(Permission.PlayersWrite);
  const roleOptions = isAdmin ? userRoleOptions : adminCreateRoleOptions;

  const editPlayerFormFields = useMemo<FieldConfig<UpdatePlayerFormData>[]>(
    () => [
      {
        name: 'email',
        label: 'Email Address',
        type: 'text',
        placeholder: 'e.g., player@example.com',
        required: false,
        gridCols: 6,
        helperText: 'Must be a valid and unique email address',
      },
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
        name: 'role',
        label: 'Role',
        type: 'select',
        gridCols: 6,
        options: roleOptions,
        helperText: 'User role in the system',
      },
      {
        name: 'isActive',
        label: 'Account Active',
        type: 'switch',
        gridCols: 6,
        helperText: 'Toggle to activate/deactivate the account',
      },
    ],
    [roleOptions],
  );
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { data: player, isLoading } = useQuery({
    queryKey: ['player', id],
    queryFn: () => getPlayerById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30,
    refetchOnWindowFocus: true,
    refetchOnMount: true,
  });

  const handleSubmit = async (data: UpdatePlayerFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.email || !data.username) {
      setError('Email and username are required');
      setIsSubmitting(false);
      return;
    }

    try {
      await updatePlayer(id as string, {
        email: data.email,
        username: data.username,
        firstName: data.firstName || undefined,
        lastName: data.lastName || undefined,
        role: data.role as UserRole,
        isActive: data.isActive,
      });

      setSuccess('Player updated successfully!');

      setTimeout(() => {
        navigate('/players');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update player');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/players');
  };

  if (isLoading) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <FormSkeleton />
      </Paper>
    );
  }

  const playerData = player;
  const displayName =
    playerData?.firstName && playerData?.lastName
      ? `${playerData.firstName} ${playerData.lastName}`
      : playerData?.username;

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Edit Player
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Update details for {displayName}
        </Typography>
      </Box>

      <FormBuilder<UpdatePlayerFormData>
        fields={editPlayerFormFields}
        schema={updatePlayerSchema}
        defaultValues={{
          email: playerData?.email || '',
          username: playerData?.username || '',
          firstName: playerData?.firstName || '',
          lastName: playerData?.lastName || '',
          role: playerData?.role || 'player',
          isActive: playerData?.isActive ?? true,
        }}
        mode={canWrite ? 'edit' : 'view'}
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={isSubmitting}
        error={error}
        success={success}
        showCancel={canWrite}
        showReset={canWrite}
        submitLabel="Update Player"
        cancelLabel="Cancel"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
