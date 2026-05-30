import { Box, CircularProgress, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { GameForm } from '../../../containers/games/GameForm';
import type { GamePayload } from '../../../services/game/add';
import { getGame } from '../../../services/game/get';
import { updateGame } from '../../../services/game/update';

export default function GameDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  const { data, isLoading } = useQuery({
    queryKey: ['game', id],
    queryFn: () => getGame(id as string),
    enabled: Boolean(id),
  });

  async function handleSubmit(payload: GamePayload) {
    if (!id) return;
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);
    try {
      await updateGame(id, payload);
      setSuccess('Game updated successfully!');
      setTimeout(() => navigate('/games'), 1200);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update game');
    } finally {
      setLoading(false);
    }
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
        error={error}
        success={success}
        onSubmit={handleSubmit}
        onCancel={() => navigate('/games')}
      />
    </Paper>
  );
}
