import type { SearchOption } from '@gaming-cafe/ui';
import { type FieldConfig, FormBuilder, FormPage } from '@gaming-cafe/ui';
import { toastUtils } from '@gaming-cafe/utils';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { ActiveShiftGuard } from '../../../components/ActiveShiftGuard';
import {
  type StartSessionFormData,
  startSessionDefaultValues,
  startSessionSchema,
} from '../../../containers/sessions/schemas/session-schema';
import { useEnrichedPlayerPlans } from '../../../hooks/useEnrichedSessions';
import { DeviceStatus, getDevices } from '../../../services/devices/list';
import { PlanType } from '../../../services/plans/list';
import { getPlayers } from '../../../services/players/list';
import { startSession } from '../../../services/sessions/add';

export default function NewSessionPage() {
  const navigate = useNavigate();
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [selectedPlayer, setSelectedPlayer] = useState<SearchOption | null>(null);

  const { data: enrichedPlayerPlans } = useEnrichedPlayerPlans(
    selectedPlayer ? String(selectedPlayer.id) : undefined,
  );

  const { data: devicesData } = useQuery({
    queryKey: ['devices-operational'],
    queryFn: () =>
      getDevices({
        status: DeviceStatus.AVAILABLE,
        limit: 100,
      }),
    enabled: !!selectedPlayer?.id,
  });

  useEffect(() => {
    if (selectedPlayer?.id && enrichedPlayerPlans.length === 0)
      toastUtils.error(
        'No active player plans found for this player. Please purchase a plan first. Please click on the button below to purchase a plan.',
        {
          position: 'bottom-center',
          autoClose: 3000,
          hideProgressBar: false,
          closeOnClick: true,
          pauseOnHover: true,
          draggable: true,
          onClick: () => {
            navigate('/plan-transactions/new');
          },
        },
      );
  }, [enrichedPlayerPlans, selectedPlayer?.id, navigate]);

  const formatPlanTypeLabel = (planType?: string) => {
    if (planType === PlanType.TIME_BASED) return 'Time Plan';
    if (planType === PlanType.WEEKEND_SPECIAL) return 'Happy Hours';
    if (!planType) return 'Plan';
    return planType
      .split('_')
      .map((word) => word.charAt(0).toUpperCase() + word.slice(1))
      .join(' ');
  };

  const balanceOptions =
    enrichedPlayerPlans.map((pp) => ({
      value: pp.id,
      label: `${pp.plan?.name} - ${formatPlanTypeLabel(pp.plan?.planType)} (${pp.remainingMinutes} min remaining)`,
    })) || [];

  const deviceOptions =
    devicesData?.data?.map((device) => ({
      value: device.id,
      label: `${device.name} (${device.deviceType})`,
    })) || [];

  const fields: FieldConfig<StartSessionFormData>[] = [
    {
      name: 'playerId',
      label: 'Player',
      type: 'search',
      onSearch: async (query: string) => {
        const data = await getPlayers({
          limit: 100,
          username: query,
          isActive: 1,
          sortBy: 'username',
          sortOrder: 'ASC',
        });
        return data.data.map((player) => ({
          label: player.username,
          id: player.id,
        }));
      },
    },
    {
      name: 'balanceId',
      label: 'Player Balance',
      type: 'select',
      placeholder: 'Select a player balance',
      required: true,
      fullWidth: true,
      options: balanceOptions,
      helperText: 'Select the player balance to use for this session',
    },
    {
      name: 'deviceId',
      label: 'Device',
      type: 'select',
      placeholder: 'Select a device',
      required: true,
      fullWidth: true,
      options: deviceOptions,
      helperText: 'Select the device to use for this session',
    },
    {
      name: 'startTime',
      label: 'Start Time (Optional)',
      type: 'datetime',
      placeholder: 'Leave empty to use current time',
      fullWidth: true,
      helperText: 'Optional: Specify a custom start time or leave empty for current time',
    },
  ];

  const handleSubmit = async (data: StartSessionFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);

    try {
      await startSession({
        balanceId: data.balanceId,
        deviceId: data.deviceId,
      });

      setSuccess('Session started successfully!');

      setTimeout(() => {
        navigate('/sessions');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to start session');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/sessions');
  };

  const onPlayerSearchComplete = (data: SearchOption) => {
    setSelectedPlayer(data);
  };

  return (
    <ActiveShiftGuard>
      <FormPage
        title="Start New Session"
        description="Start a new gaming session for a player"
        backTo="/sessions"
        backLabel="Back to sessions"
      >
        <FormBuilder<StartSessionFormData>
          fields={fields}
          schema={startSessionSchema}
          defaultValues={startSessionDefaultValues}
          mode="add"
          onSubmit={handleSubmit}
          onCancel={handleCancel}
          loading={isSubmitting}
          error={error}
          success={success}
          showCancel
          submitLabel="Start Session"
          cancelLabel="Cancel"
          buttonAlign="right"
          spacing={3}
          onSearchComplete={onPlayerSearchComplete}
        />
      </FormPage>
    </ActiveShiftGuard>
  );
}
