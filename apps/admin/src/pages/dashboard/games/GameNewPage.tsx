import { Box, Paper, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { GameForm } from '../../../containers/games/GameForm';
import { addGame, type GamePayload } from '../../../services/game/add';

export default function GameNewPage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  async function handleSubmit(payload: GamePayload) {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);
    try {
      await addGame(payload);
      setSuccess('Game created successfully!');
      setTimeout(() => navigate('/games'), 1200);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create game');
    } finally {
      setLoading(false);
    }
  }

  return (
    <Paper elevation={0} sx={{ p: 4 }}>
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Add New Game
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Upload branding assets and configure how this game appears on the kiosk.
        </Typography>
      </Box>
      <GameForm
        submitLabel="Create Game"
        loading={loading}
        error={error}
        success={success}
        onSubmit={handleSubmit}
        onCancel={() => navigate('/games')}
      />
    </Paper>
  );
}
