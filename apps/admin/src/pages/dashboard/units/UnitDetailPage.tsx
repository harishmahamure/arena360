import { FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  type CreateUnitFormData,
  createUnitSchema,
} from '../../../containers/units/schemas/unit-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { getUnitById } from '../../../services/units/getById';
import { updateUnit } from '../../../services/units/update';
import { unitFormFields } from './UnitNewPage';

export default function EditUnitPage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const { can } = usePermissions();
  const canWrite = can(Permission.UnitsWrite);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { data: unit, isLoading } = useQuery({
    queryKey: ['unit', id],
    queryFn: () => getUnitById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30, // 30 seconds
    refetchOnWindowFocus: true,
    refetchOnMount: true,
    refetchOnReconnect: true,
    refetchInterval: 1000 * 30,
    refetchIntervalInBackground: true,
  });

  const handleSubmit = async (data: CreateUnitFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);
    if (!data.name || !data.abbreviation) {
      setError('Name and abbreviation are required');
      setIsSubmitting(false);
      return;
    }
    try {
      await updateUnit(id as string, {
        name: data.name,
        abbreviation: data.abbreviation,
        type: data.type,
        description: data.description,
        isActive: data.isActive,
      });
      setSuccess('Unit updated successfully!');
      navigate('/units');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update unit');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/units');
  };

  if (isLoading) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <FormSkeleton />
      </Paper>
    );
  }

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Update Unit{' '}
        </Typography>
        <Typography variant="body2" color="text.secondary">
          {unit?.name} - Update the unit details.
        </Typography>
      </Box>

      <FormBuilder<CreateUnitFormData>
        fields={unitFormFields}
        schema={createUnitSchema}
        defaultValues={{
          name: unit?.name,
          abbreviation: unit?.abbreviation,
          type: unit?.type as CreateUnitFormData['type'],
          description: unit?.description,
          isActive: unit?.isActive,
        }}
        mode={canWrite ? 'edit' : 'view'}
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={isSubmitting}
        error={error}
        success={success}
        showCancel={canWrite}
        showReset={canWrite}
        submitLabel="Update Unit"
        cancelLabel="Cancel"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
