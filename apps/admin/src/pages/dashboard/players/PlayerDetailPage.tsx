import type { UserRole } from '@gaming-cafe/contracts';
import { type FieldConfig, FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { Box, Button, Divider, Paper, TextField, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import TotpQrCode from '../../../components/TotpQrCode';
import {
  adminCreateRoleOptions,
  type UpdatePlayerFormData,
  updatePlayerSchema,
  userRoleOptions,
} from '../../../containers/players/schemas/player-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getPlayerById } from '../../../services/players/getById';
import { updatePlayer } from '../../../services/players/update';
import { disableTotp, setupTotp, verifyTotpSetup } from '../../../services/users/totp';

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
  const [totpSecret, setTotpSecret] = useState('');
  const [totpUri, setTotpUri] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [totpEnabled, setTotpEnabled] = useState(false);
  const [totpLoading, setTotpLoading] = useState(false);

  const { data: player, isLoading } = useQuery({
    queryKey: ['player', id],
    queryFn: () => getPlayerById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30,
    refetchOnWindowFocus: true,
    refetchOnMount: true,
  });

  useEffect(() => {
    if (player?.totpEnabled !== undefined) {
      setTotpEnabled(Boolean(player.totpEnabled));
    }
  }, [player?.totpEnabled]);

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

  const handleSetupTotp = async () => {
    if (!id) return;
    setTotpLoading(true);
    try {
      const response = await setupTotp(id);
      setTotpSecret(response.secret);
      setTotpUri(response.otpauthUri);
      setTotpEnabled(response.totpEnabled);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to setup TOTP');
    } finally {
      setTotpLoading(false);
    }
  };

  const handleVerifyTotp = async () => {
    if (!id) return;
    setTotpLoading(true);
    try {
      const response = await verifyTotpSetup(id, totpCode);
      setTotpEnabled(response.totpEnabled);
      setTotpCode('');
      setSuccess('TOTP enabled successfully');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to verify TOTP');
    } finally {
      setTotpLoading(false);
    }
  };

  const handleDisableTotp = async () => {
    if (!id) return;
    setTotpLoading(true);
    try {
      await disableTotp(id);
      setTotpEnabled(false);
      setTotpSecret('');
      setTotpUri('');
      setSuccess('TOTP disabled');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to disable TOTP');
    } finally {
      setTotpLoading(false);
    }
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

      {isAdmin && playerData?.role === 'staff' && (
        <Box sx={{ mt: 4 }}>
          <Divider sx={{ mb: 3 }} />
          <Typography variant="h6" fontWeight={600} gutterBottom>
            Staff TOTP Setup
          </Typography>
          <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
            Configure authenticator TOTP for shift handover validation.
          </Typography>
          <Typography variant="body2" sx={{ mb: 2 }}>
            Status: {totpEnabled ? 'Enabled' : 'Not enabled'}
          </Typography>
          {!totpSecret && !totpEnabled && (
            <Button variant="outlined" onClick={handleSetupTotp} disabled={totpLoading}>
              Generate TOTP Secret
            </Button>
          )}
          {totpSecret && !totpEnabled && (
            <Box sx={{ display: 'grid', gap: 2, maxWidth: 480 }}>
              {totpUri && (
                <TotpQrCode
                  value={totpUri}
                  accountLabel={playerData?.username}
                  secret={totpSecret}
                />
              )}
              <TextField
                label="Verification Code"
                value={totpCode}
                onChange={(event) => setTotpCode(event.target.value)}
                inputProps={{ maxLength: 6 }}
              />
              <Box sx={{ display: 'flex', gap: 1 }}>
                <Button variant="contained" onClick={handleVerifyTotp} disabled={totpLoading}>
                  Verify & Enable
                </Button>
              </Box>
            </Box>
          )}
          {totpEnabled && (
            <Button
              color="error"
              variant="outlined"
              onClick={handleDisableTotp}
              disabled={totpLoading}
            >
              Disable TOTP
            </Button>
          )}
        </Box>
      )}
    </Paper>
  );
}
