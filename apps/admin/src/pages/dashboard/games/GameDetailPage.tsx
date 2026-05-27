import { FormBuilder, FormSkeleton } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  type AgeRatingType,
  type CreateGameFormData,
  createGameSchema,
  type GameCategoryType,
  type GamePlatformType,
} from '../../../containers/games/schemas/game-schema';
import { getGameById } from '../../../services/games/getById';
import { updateGame } from '../../../services/games/update';
import { gameFormFields } from './GameNewPage';

export default function EditGamePage() {
  const navigate = useNavigate();
  const { id } = useParams();
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);

  const { data: game, isLoading } = useQuery({
    queryKey: ['game', id],
    queryFn: () => getGameById(id as string),
    enabled: !!id,
    staleTime: 1000 * 30, // 30 seconds
    refetchOnWindowFocus: true,
    refetchOnMount: true,
    refetchOnReconnect: true,
    refetchInterval: 1000 * 30,
    refetchIntervalInBackground: true,
  });

  const handleSubmit = async (data: CreateGameFormData) => {
    setIsSubmitting(true);
    setError(undefined);
    setSuccess(undefined);
    if (!data.title) {
      setError('Game title is required');
      setIsSubmitting(false);
      return;
    }
    try {
      await updateGame(id as string, data);
      setSuccess('Game updated successfully!');
      navigate('/games');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to update game');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleCancel = () => {
    navigate('/games');
  };

  if (isLoading) {
    return (
      <Paper elevation={0} sx={{ p: 4 }}>
        <FormSkeleton />
      </Paper>
    );
  }

  // Convert tags array to comma-separated string for the form
  const tagsString = game?.tags
    ? Array.isArray(game.tags)
      ? game.tags.join(', ')
      : game.tags
    : '';

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Update Game{' '}
        </Typography>
        <Typography variant="body2" color="text.secondary">
          {game?.title} - Update the game details.
        </Typography>
      </Box>

      <FormBuilder<CreateGameFormData>
        fields={gameFormFields}
        schema={createGameSchema}
        defaultValues={{
          title: game?.title || '',
          description: game?.description || '',
          genre: game?.genre || '',
          category: (game?.category as unknown as GameCategoryType) || undefined,
          platform: (game?.platform as unknown as GamePlatformType) || undefined,
          isActive: game?.isActive ?? true,
          imageUrl: game?.imageUrl || '',
          videoUrl: game?.videoUrl || '',
          trailerUrl: game?.trailerUrl || '',
          developer: game?.developer || '',
          publisher: game?.publisher || '',
          releaseDate: game?.releaseDate ? new Date(game.releaseDate) : null,
          isMultiplayer: game?.isMultiplayer ?? false,
          tags: tagsString,
          ageRating: (game?.ageRating as unknown as AgeRatingType) || undefined,
          minPlayers: game?.minPlayers || 1,
          maxPlayers: game?.maxPlayers || 1,
        }}
        mode="edit"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={isSubmitting}
        error={error}
        success={success}
        showCancel
        showReset
        submitLabel="Update Game"
        cancelLabel="Cancel"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
