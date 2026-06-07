import { Autocomplete, Card, CardContent, TextField, Typography } from '@mui/material';
import { useEffect, useState } from 'react';
import { getPlayers } from '../../services/players/list';

export interface PosPlayer {
  id: string;
  username: string;
}

export interface PosPlayerPickerProps {
  value: PosPlayer | null;
  onChange: (player: PosPlayer | null) => void;
  helperText?: string;
}

export function PosPlayerPicker({
  value,
  onChange,
  helperText = 'Player account to charge; required before checkout',
}: PosPlayerPickerProps) {
  const [playerOptions, setPlayerOptions] = useState<PosPlayer[]>([]);
  const [playerInputValue, setPlayerInputValue] = useState('');
  const [playerLoading, setPlayerLoading] = useState(false);

  useEffect(() => {
    if (playerInputValue.length < 2) {
      setPlayerOptions([]);
      return;
    }

    const searchPlayers = async () => {
      setPlayerLoading(true);
      try {
        const data = await getPlayers({
          limit: 100,
          username: playerInputValue,
          isActive: 1,
          sortBy: 'username',
          sortOrder: 'ASC',
        });
        setPlayerOptions(
          data.data.map((player) => ({
            id: player.id,
            username: player.username,
          })),
        );
      } catch (_err) {
        // ignore search errors
      } finally {
        setPlayerLoading(false);
      }
    };

    const timeoutId = setTimeout(searchPlayers, 300);
    return () => clearTimeout(timeoutId);
  }, [playerInputValue]);

  return (
    <Card variant="outlined" sx={{ mb: 3 }}>
      <CardContent>
        <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
          Select player
        </Typography>
        <Autocomplete
          options={playerOptions}
          getOptionLabel={(option) => option.username}
          value={value}
          onChange={(_, newValue) => onChange(newValue)}
          inputValue={playerInputValue}
          onInputChange={(_, newValue) => setPlayerInputValue(newValue)}
          loading={playerLoading}
          renderInput={(params) => (
            <TextField
              {...params}
              placeholder="Search player by username..."
              fullWidth
              helperText={helperText}
            />
          )}
          renderOption={(props, option) => (
            <li {...props} key={option.id}>
              <Typography variant="body1">{option.username}</Typography>
            </li>
          )}
        />
      </CardContent>
    </Card>
  );
}
