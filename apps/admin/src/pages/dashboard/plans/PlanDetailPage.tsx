import { PlanType, type PlanTypeValue } from '@gaming-cafe/contracts';
import { FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  type CreatePlanFormData,
  createPlanSchema,
} from '../../../containers/plans/schemas/plan.schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import type { CreatePlanPayload } from '../../../services/plans/add';
import { getPlanById } from '../../../services/plans/getById';
import { updatePlan } from '../../../services/plans/update';
import { planFormFields } from './PlanNewPage';

function normalizeTime(value?: string): string | undefined {
  if (!value) return undefined;
  return value.length === 5 ? `${value}:00` : value;
}

export default function EditPlanPage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const { can } = usePermissions();
  const canWrite = can(Permission.PlansWrite);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { data: plan, isLoading } = useQuery({
    queryKey: ['plan', id],
    queryFn: () => getPlanById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30,
    refetchOnWindowFocus: true,
    refetchOnMount: true,
    refetchOnReconnect: true,
    refetchInterval: 1000 * 30,
    refetchIntervalInBackground: true,
  });

  const handleSubmit = async (data: CreatePlanFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);

    if (!data.name || !data.price || !data.planType) {
      setError('Name, price, and plan type are required');
      setIsSubmitting(false);
      return;
    }

    try {
      const payload: Partial<CreatePlanPayload> &
        Pick<CreatePlanFormData, 'deviceType' | 'deviceSubType'> = {
        name: data.name,
        description: data.description,
        price: data.price,
        planType: data.planType as PlanTypeValue,
        validityDays: data.validityDays ?? undefined,
        isActive: data.isActive,
        deviceType: data.deviceType ?? undefined,
        deviceSubType: data.deviceSubType ?? undefined,
      };

      if (data.timeCredits) payload.timeCredits = data.timeCredits;
      if (data.timeWindowStart) payload.timeWindowStart = data.timeWindowStart;
      if (data.timeWindowEnd) payload.timeWindowEnd = data.timeWindowEnd;
      if (data.allowedDays?.length) payload.allowedDays = data.allowedDays;
      if (data.allowedMonths?.length) payload.allowedMonths = data.allowedMonths;
      payload.dynamicDeductionEnabled = data.dynamicDeductionEnabled ?? false;
      if (data.dynamicDeductionEnabled) {
        payload.deductionProfile = {
          peakWindowStart: normalizeTime(data.peakWindowStart) ?? '',
          peakWindowEnd: normalizeTime(data.peakWindowEnd) ?? '',
          peakRatio: data.peakRatio ?? 1.5,
          lowWindowStart: normalizeTime(data.lowWindowStart) ?? '',
          lowWindowEnd: normalizeTime(data.lowWindowEnd) ?? '',
          lowRatio: data.lowRatio ?? 0.8,
        };
      }

      await updatePlan(id as string, payload);
      setSuccess('Plan updated successfully!');

      setTimeout(() => {
        navigate('/plans');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update plan');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/plans');
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
          Update Plan
        </Typography>
        <Typography variant="body2" color="text.secondary">
          {plan?.name} - Update the plan details
        </Typography>
      </Box>

      <FormBuilder<CreatePlanFormData>
        fields={planFormFields}
        schema={createPlanSchema}
        defaultValues={{
          name: plan?.name || '',
          description: plan?.description || '',
          price: plan?.price ? parseFloat(plan.price) : 0,
          planType:
            plan?.planType === PlanType.WEEKEND_SPECIAL
              ? PlanType.WEEKEND_SPECIAL
              : PlanType.TIME_BASED,
          validityDays: plan?.validityDays || 7,
          timeWindowStart: plan?.timeWindowStart,
          timeWindowEnd: plan?.timeWindowEnd,
          timeCredits: plan?.timeCredits,
          isActive: plan?.isActive ?? true,
          deviceSubType: (plan?.deviceSubType ?? undefined) as CreatePlanFormData['deviceSubType'],
          deviceType: (plan?.deviceType ?? undefined) as CreatePlanFormData['deviceType'],
          allowedDays: plan?.allowedDays ?? undefined,
          allowedMonths: plan?.allowedMonths ?? undefined,
          dynamicDeductionEnabled: plan?.dynamicDeductionEnabled ?? false,
          peakWindowStart: plan?.deductionProfile?.peakWindowStart,
          peakWindowEnd: plan?.deductionProfile?.peakWindowEnd,
          peakRatio: plan?.deductionProfile?.peakRatio ?? 1.5,
          lowWindowStart: plan?.deductionProfile?.lowWindowStart,
          lowWindowEnd: plan?.deductionProfile?.lowWindowEnd,
          lowRatio: plan?.deductionProfile?.lowRatio ?? 0.8,
        }}
        mode={canWrite ? 'edit' : 'view'}
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={isSubmitting}
        error={error}
        success={success}
        showCancel={canWrite}
        showReset={canWrite}
        submitLabel="Update Plan"
        cancelLabel="Cancel"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
