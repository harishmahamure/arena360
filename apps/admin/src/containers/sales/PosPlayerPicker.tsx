import { Autocomplete, Card, CardContent, Chip, Stack, TextField, Typography } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { Permission, usePermissions } from '../../hooks/usePermissions';
import { getPlayers } from '../../services/players/list';
import { getSessions } from '../../services/sessions/list';

export interface PosPlayer {
  id: string;
  username: string;
}

type PlayerOption = PosPlayer & { station?: string };

export interface PosPlayerPickerProps {
  value: PosPlayer | null;
  onChange: (player: PosPlayer | null) => void;
  helperText?: string;
  disabled?: boolean;
}

export function PosPlayerPicker({
  value,
  onChange,
  helperText = 'Tap a player in session, or type 2+ letters to search everyone',
  disabled = false,
}: PosPlayerPickerProps) {
  const { can } = usePermissions();
  const [searchResults, setSearchResults] = useState<PlayerOption[]>([]);
  const [playerInputValue, setPlayerInputValue] = useState('');
  const [playerLoading, setPlayerLoading] = useState(false);

  const { data: activeSessions } = useQuery({
    queryKey: ['pos-in-session-players'],
    queryFn: () => getSessions({ isActive: 1, limit: 100 }),
    enabled: can(Permission.SessionsRead),
    refetchInterval: 30_000,
  });

  const inSession = useMemo(() => {
    const players = new Map<string, PlayerOption>();
    for (const session of activeSessions?.data ?? []) {
      const player = session.balance?.player;
      if (player && !players.has(player.id))
        players.set(player.id, {
          id: player.id,
          username: player.username,
          station: session.device?.name,
        });
    }
    return [...players.values()].sort((a, b) => a.username.localeCompare(b.username));
  }, [activeSessions]);

  const searching = playerInputValue.trim().length >= 2 && playerInputValue !== value?.username;

  useEffect(() => {
    if (!searching) {
      setSearchResults([]);
      return;
    }

    const searchPlayers = async () => {
      setPlayerLoading(true);
      try {
        const data = await getPlayers({
          limit: 100,
          username: playerInputValue.trim(),
          isActive: 1,
          sortBy: 'username',
          sortOrder: 'ASC',
        });
        const stations = new Map(inSession.map((player) => [player.id, player.station]));
        setSearchResults(
          data.data.map((player) => ({
            id: player.id,
            username: player.username,
            station: stations.get(player.id),
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
  }, [playerInputValue, searching, inSession]);

  return (
    <Card variant="outlined" sx={{ mb: 3 }}>
      <CardContent>
        <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
          Select player
        </Typography>
        {inSession.length > 0 && (
          <>
            <Typography variant="caption" color="text.secondary">
              In session now
            </Typography>
            <Stack direction="row" flexWrap="wrap" gap={1} sx={{ mt: 0.5, mb: 2 }}>
              {inSession.map((player) => (
                <Chip
                  key={player.id}
                  label={
                    player.station ? `${player.username} · ${player.station}` : player.username
                  }
                  color={value?.id === player.id ? 'primary' : 'default'}
                  variant={value?.id === player.id ? 'filled' : 'outlined'}
                  disabled={disabled}
                  onClick={() => onChange(value?.id === player.id ? null : player)}
                />
              ))}
            </Stack>
          </>
        )}
        <Autocomplete<PlayerOption>
          options={searching ? searchResults : inSession}
          filterOptions={searching ? (options) => options : undefined}
          getOptionLabel={(option) => option.username}
          isOptionEqualToValue={(option, selected) => option.id === selected.id}
          value={value}
          onChange={(_, newValue) => onChange(newValue)}
          inputValue={playerInputValue}
          onInputChange={(_, newValue) => setPlayerInputValue(newValue)}
          loading={playerLoading}
          disabled={disabled}
          noOptionsText={searching ? 'No players found' : 'Type 2+ letters to search players'}
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
              {option.station && (
                <Typography variant="caption" color="success.main" sx={{ ml: 1 }}>
                  in session · {option.station}
                </Typography>
              )}
            </li>
          )}
        />
      </CardContent>
    </Card>
  );
}
