import { PlaceOutlined } from '@mui/icons-material';
import {
  Box,
  Button,
  Card,
  CardContent,
  FormControlLabel,
  Radio,
  RadioGroup,
  Stack,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import type { VenueLocation } from '../services/config';

export default function LocationChoice({
  locations,
  onChoose,
  title = 'Choose your location',
  description = 'Select the venue where you will work before opening the dashboard.',
  action = 'Open dashboard',
}: {
  locations: VenueLocation[];
  onChoose: (id: string) => void;
  title?: string;
  description?: string;
  action?: string;
}) {
  const [chosen, setChosen] = useState('');
  return (
    <Box sx={{ maxWidth: 560, mx: 'auto', py: { xs: 2, sm: 5 } }}>
      <Card variant="outlined">
        <CardContent sx={{ p: { xs: 2, sm: 4 } }}>
          <Stack gap={3}>
            <PlaceOutlined color="primary" sx={{ fontSize: 36 }} />
            <Typography variant="h4" component="h1" fontWeight={700}>
              {title}
            </Typography>
            <Typography color="text.secondary">{description}</Typography>
            <RadioGroup
              aria-label="Operating location"
              value={chosen}
              onChange={(event) => setChosen(event.target.value)}
              sx={{ maxHeight: 360, overflowY: 'auto', gap: 1 }}
            >
              {locations.map((location) => (
                <FormControlLabel
                  key={location.id}
                  value={location.id}
                  control={<Radio />}
                  label={
                    <Box sx={{ py: 1 }}>
                      <Typography fontWeight={600}>{location.name}</Typography>
                      <Typography variant="body2" color="text.secondary">
                        {location.timezone} · {location.currency}
                      </Typography>
                    </Box>
                  }
                  sx={{
                    m: 0,
                    p: 1,
                    border: 1,
                    borderColor: chosen === location.id ? 'primary.main' : 'divider',
                    borderRadius: 2,
                  }}
                />
              ))}
            </RadioGroup>
            <Button
              variant="contained"
              size="large"
              disabled={!locations.some((location) => location.id === chosen)}
              onClick={() => onChoose(chosen)}
            >
              {action}
            </Button>
          </Stack>
        </CardContent>
      </Card>
    </Box>
  );
}
