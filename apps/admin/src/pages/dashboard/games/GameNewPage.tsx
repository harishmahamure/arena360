import { type FieldConfig, FormBuilder } from '@gaming-cafe/ui';
import { Box, Paper, Typography } from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  ageRatingOptions,
  type CreateGameFormData,
  createGameDefaultValues,
  createGameSchema,
  gameCategoryOptions,
  gamePlatformOptions,
} from '../../../../src/containers/games/schemas/game-schema';
import { addGame } from '../../../services/games/add';

export const gameFormFields: FieldConfig<CreateGameFormData>[] = [
  {
    name: 'title',
    label: 'Game Title',
    type: 'text',
    placeholder: 'e.g., Call of Duty: Modern Warfare',
    required: true,
    gridCols: 6,
    helperText: 'Enter the game title (max 200 characters)',
  },
  {
    name: 'genre',
    label: 'Genre',
    type: 'text',
    required: false,
    placeholder: 'e.g., First-Person Shooter',
    gridCols: 6,
    helperText: 'Game genre (max 100 characters)',
  },
  {
    name: 'description',
    label: 'Description',
    type: 'textarea',
    placeholder: 'Enter game description...',
    fullWidth: true,
    rows: 3,
    helperText: 'Optional game description',
  },
  {
    name: 'category',
    label: 'Category',
    type: 'select',
    required: false,
    gridCols: 4,
    options: gameCategoryOptions,
    helperText: 'Select game category',
  },
  {
    name: 'platform',
    label: 'Platform',
    type: 'select',
    required: false,
    gridCols: 4,
    options: gamePlatformOptions,
    helperText: 'Select gaming platform',
  },
  {
    name: 'ageRating',
    label: 'Age Rating',
    type: 'select',
    required: false,
    gridCols: 4,
    options: ageRatingOptions,
    helperText: 'Select age rating',
  },
  {
    name: 'developer',
    label: 'Developer',
    type: 'text',
    placeholder: 'e.g., Infinity Ward',
    gridCols: 6,
    helperText: 'Game developer studio',
  },
  {
    name: 'publisher',
    label: 'Publisher',
    type: 'text',
    placeholder: 'e.g., Activision',
    gridCols: 6,
    helperText: 'Game publisher',
  },
  {
    name: 'releaseDate',
    label: 'Release Date',
    type: 'date',
    gridCols: 4,
    helperText: 'Game release date',
  },
  {
    name: 'minPlayers',
    label: 'Min Players',
    type: 'number',
    placeholder: '1',
    gridCols: 4,
    min: 1,
    helperText: 'Minimum number of players',
  },
  {
    name: 'maxPlayers',
    label: 'Max Players',
    type: 'number',
    placeholder: '1',
    gridCols: 4,
    min: 1,
    helperText: 'Maximum number of players',
  },
  {
    name: 'imageUrl',
    label: 'Image URL',
    type: 'text',
    placeholder: 'https://example.com/image.jpg',
    gridCols: 4,
    helperText: 'Game cover image URL',
  },
  {
    name: 'videoUrl',
    label: 'Video URL',
    type: 'text',
    placeholder: 'https://example.com/video.mp4',
    gridCols: 4,
    helperText: 'Game video URL',
  },
  {
    name: 'trailerUrl',
    label: 'Trailer URL',
    type: 'text',
    placeholder: 'https://example.com/trailer.mp4',
    gridCols: 4,
    helperText: 'Game trailer URL',
  },
  {
    name: 'tags',
    label: 'Tags',
    type: 'text',
    placeholder: 'action, adventure, rpg',
    fullWidth: true,
    helperText: 'Comma-separated tags for categorizing the game',
  },
  {
    name: 'isMultiplayer',
    label: 'Multiplayer Game',
    type: 'switch',
    gridCols: 6,
    helperText: 'Toggle if the game supports multiplayer',
  },
  {
    name: 'isActive',
    label: 'Active (Available to play)',
    type: 'switch',
    gridCols: 6,
    helperText: 'Toggle to make game available',
  },
];

export default function AddNewGamePage() {
  const navigate = useNavigate();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();

  const handleSubmit = async (data: CreateGameFormData) => {
    setLoading(true);
    setError(undefined);
    setSuccess(undefined);
    if (!data.title) {
      setError('Game title is required');
      setLoading(false);
      return;
    }
    try {
      // Transform tags from comma-separated string to array
      const tagsArray = data.tags
        ? data.tags
            .split(',')
            .map((tag) => tag.trim())
            .filter(Boolean)
        : undefined;

      await addGame({
        title: data.title,
        description: data.description || undefined,
        genre: data.genre || undefined,
        category: data.category || undefined,
        platform: data.platform || undefined,
        isActive: data.isActive,
        imageUrl: data.imageUrl || undefined,
        videoUrl: data.videoUrl || undefined,
        trailerUrl: data.trailerUrl || undefined,
        developer: data.developer || undefined,
        publisher: data.publisher || undefined,
        releaseDate: data.releaseDate ? new Date(data.releaseDate).toISOString() : undefined,
        isMultiplayer: data.isMultiplayer,
        tags: tagsArray,
        ageRating: data.ageRating || undefined,
        minPlayers: data.minPlayers || undefined,
        maxPlayers: data.maxPlayers || undefined,
      });

      setSuccess('Game created successfully!');

      // Navigate back to games list after a short delay
      setTimeout(() => {
        navigate('/games');
      }, 1500);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create game');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/games');
  };

  return (
    <Paper
      elevation={0}
      sx={{
        p: 4,
      }}
    >
      <Box sx={{ mb: 4 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Add New Game
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Fill in the details below to create a new game
        </Typography>
      </Box>

      <FormBuilder<CreateGameFormData>
        fields={gameFormFields}
        schema={createGameSchema}
        defaultValues={createGameDefaultValues}
        mode="add"
        onSubmit={handleSubmit}
        onCancel={handleCancel}
        loading={loading}
        error={error}
        success={success}
        showCancel
        showReset
        submitLabel="Create Game"
        cancelLabel="Cancel"
        resetLabel="Reset Form"
        buttonAlign="right"
        spacing={3}
      />
    </Paper>
  );
}
