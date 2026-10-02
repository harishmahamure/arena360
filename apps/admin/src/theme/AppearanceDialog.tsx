import { Close, PaletteOutlined } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  IconButton,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useAppearance } from './AppearanceProvider';

export default function AppearanceDialog() {
  const [open, setOpen] = useState(false);
  const { preferences, update, reset } = useAppearance();
  const [accent, setAccent] = useState(preferences.accent);
  const valid = /^#[0-9a-f]{6}$/i.test(accent);
  return (
    <>
      <Tooltip title="Appearance">
        <IconButton
          aria-label="Customize appearance"
          onClick={() => {
            setAccent(preferences.accent);
            setOpen(true);
          }}
        >
          <PaletteOutlined fontSize="small" />
        </IconButton>
      </Tooltip>
      <Dialog open={open} onClose={() => setOpen(false)}>
        <DialogTitle
          sx={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}
        >
          Make it your workspace
          <IconButton aria-label="Close appearance" onClick={() => setOpen(false)}>
            <Close />
          </IconButton>
        </DialogTitle>
        <DialogContent>
          <Typography color="text.secondary" variant="body2" sx={{ mb: 3 }}>
            Changes apply instantly across the panel and are saved for your account on this browser.
          </Typography>
          <Stack spacing={3}>
            <TextField
              select
              label="Color mode"
              value={preferences.mode}
              onChange={(e) => update({ mode: e.target.value as typeof preferences.mode })}
            >
              <MenuItem value="system">Use device setting</MenuItem>
              <MenuItem value="light">Light</MenuItem>
              <MenuItem value="dark">Dark</MenuItem>
            </TextField>
            <Box>
              <Typography variant="subtitle2" sx={{ mb: 1 }}>
                Accent color
              </Typography>
              <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
                {['#176b51', '#365edc', '#7444bc', '#ab3e70', '#a34b14', '#087e8b'].map((color) => (
                  <IconButton
                    key={color}
                    aria-label={`Use accent ${color}`}
                    aria-pressed={preferences.accent === color}
                    onClick={() => {
                      update({ accent: color });
                      setAccent(color);
                    }}
                    sx={{
                      bgcolor: color,
                      width: 36,
                      height: 36,
                      border: preferences.accent === color ? '3px solid' : '3px solid transparent',
                      borderColor: preferences.accent === color ? 'text.primary' : undefined,
                      '&:hover': { bgcolor: color, opacity: 0.8 },
                    }}
                  />
                ))}
              </Stack>
            </Box>
            <TextField
              label="Custom accent (hex)"
              value={accent}
              error={!valid}
              helperText={
                valid
                  ? 'For example, #365edc. Button text adjusts for contrast.'
                  : 'Enter a six-digit hex color, such as #365edc.'
              }
              onChange={(e) => {
                setAccent(e.target.value);
                if (/^#[0-9a-f]{6}$/i.test(e.target.value)) update({ accent: e.target.value });
              }}
            />
            <Stack direction="row" spacing={2}>
              <TextField
                fullWidth
                select
                label="Density"
                value={preferences.density}
                onChange={(e) => update({ density: e.target.value as typeof preferences.density })}
              >
                <MenuItem value="comfortable">Comfortable</MenuItem>
                <MenuItem value="compact">Compact</MenuItem>
              </TextField>
              <TextField
                fullWidth
                select
                label="Corners"
                value={preferences.corners}
                onChange={(e) => update({ corners: e.target.value as typeof preferences.corners })}
              >
                <MenuItem value="rounded">Rounded</MenuItem>
                <MenuItem value="square">Square</MenuItem>
              </TextField>
            </Stack>
            <Alert severity="info">
              Appearance does not change organization configuration or pricing.
            </Alert>
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button
            onClick={() => {
              reset();
              setAccent('#176b51');
            }}
          >
            Reset defaults
          </Button>
          <Button variant="contained" onClick={() => setOpen(false)}>
            Done
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}
