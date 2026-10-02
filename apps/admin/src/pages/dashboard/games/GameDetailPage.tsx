import { useAsyncAction } from '@gaming-cafe/utils';
import { Box, CircularProgress, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useNavigate, useParams } from 'react-router-dom';
import { GameForm } from '../../../containers/games/GameForm';
import type { GamePayload } from '../../../services/game/add';
import { getGame } from '../../../services/game/get';
import { updateGame } from '../../../services/game/update';

export default function GameDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { loading, succeeded, failed, errorMessage, run } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });

  const { data, isLoading } = useQuery({
    queryKey: ['game', id],
    queryFn: () => getGame(id as string),
    enabled: Boolean(id),
  });

  function handleSubmit(payload: GamePayload) {
    if (!id) return;
    void run(async () => {
      await updateGame(id, payload);
      setTimeout(() => navigate('/games'), 1200);
    });
  }

  if (isLoading || !data) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', py: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Paper elevation={0} sx={{ p: 4 }}>
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Edit Game
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Update branding assets and kiosk visibility.
        </Typography>
      </Box>
      <GameForm
        initial={data}
        submitLabel="Save Changes"
        loading={loading}
        submitSuccess={succeeded}
        submitSuccessLabel="Game saved"
        submitError={failed}
        submitErrorLabel={errorMessage ?? 'Failed to update game'}
        onSubmit={handleSubmit}
        onCancel={() => navigate('/games')}
      />
    </Paper>
  );
}
