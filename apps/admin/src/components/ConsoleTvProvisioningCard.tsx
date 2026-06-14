import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import LoginIcon from '@mui/icons-material/Login';
import TvIcon from '@mui/icons-material/Tv';
import {
  Alert,
  Box,
  List,
  ListItem,
  ListItemIcon,
  ListItemText,
  Paper,
  Typography,
} from '@mui/material';

export interface ConsoleTvProvisioningCardProps {
  deviceName?: string;
  registrationStatus?: string;
}

const PROVISIONING_STEPS = [
  {
    icon: <TvIcon fontSize="small" />,
    primary: 'Open the Console TV app on the station',
    secondary: 'Launch Arena360 Console TV on the PlayStation station you want to register.',
  },
  {
    icon: <LoginIcon fontSize="small" />,
    primary: 'Sign in as admin on the device',
    secondary: 'Use your admin username, password, and authenticator code if TOTP is enabled.',
  },
  {
    icon: <CheckCircleOutlineIcon fontSize="small" />,
    primary: 'Name the PlayStation station and confirm',
    secondary:
      'Enter station name, PS5 or PS4 type, sub-type, and location. The TV completes provisioning automatically.',
  },
] as const;

export function ConsoleTvProvisioningCard({
  deviceName,
  registrationStatus,
}: ConsoleTvProvisioningCardProps) {
  const isRegistered = registrationStatus === 'registered';

  if (isRegistered) {
    return (
      <Alert severity="success" sx={{ mb: 3 }}>
        This PlayStation station is registered and ready for admin-started sessions.
      </Alert>
    );
  }

  return (
    <Paper variant="outlined" sx={{ p: 3, mb: 3 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 1, mb: 1 }}>
        <TvIcon color="primary" />
        <Typography variant="h6" fontWeight={600}>
          Console TV provisioning
        </Typography>
      </Box>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        {deviceName
          ? `Provision "${deviceName}" from the TV itself — no device ID or admin SSO button is needed.`
          : 'Provision this PlayStation station from the TV itself — no device ID or admin SSO button is needed.'}
      </Typography>

      <Alert severity="info" sx={{ mb: 2 }}>
        Ensure device type, sub type, and location below match what you enter on the TV. An admin
        must be physically present at the station to complete provisioning.
      </Alert>

      <List dense disablePadding>
        {PROVISIONING_STEPS.map((step) => (
          <ListItem key={step.primary} disableGutters sx={{ alignItems: 'flex-start', py: 0.75 }}>
            <ListItemIcon sx={{ minWidth: 36, mt: 0.25 }}>{step.icon}</ListItemIcon>
            <ListItemText
              primary={
                <Typography variant="body2" fontWeight={600}>
                  {step.primary}
                </Typography>
              }
              secondary={step.secondary}
            />
          </ListItem>
        ))}
      </List>

      {registrationStatus && registrationStatus !== 'registered' && (
        <Box sx={{ mt: 2 }}>
          <Typography variant="caption" color="text.secondary">
            Current status: {registrationStatus}
          </Typography>
        </Box>
      )}
    </Paper>
  );
}
