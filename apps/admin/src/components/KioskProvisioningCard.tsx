import CheckCircleOutlineIcon from '@mui/icons-material/CheckCircleOutline';
import ComputerIcon from '@mui/icons-material/Computer';
import LoginIcon from '@mui/icons-material/Login';
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

export interface KioskProvisioningCardProps {
  deviceName?: string;
  registrationStatus?: string;
}

const PROVISIONING_STEPS = [
  {
    icon: <ComputerIcon fontSize="small" />,
    primary: 'Open the kiosk app on the PC',
    secondary: 'Launch Arena360 kiosk on the station you want to register.',
  },
  {
    icon: <LoginIcon fontSize="small" />,
    primary: 'Sign in as admin on the device',
    secondary:
      'Use your admin username, password, and OTP. The kiosk stays locked during this step.',
  },
  {
    icon: <CheckCircleOutlineIcon fontSize="small" />,
    primary: 'Name the PC and confirm',
    secondary:
      'Enter a station name matching this device record. The kiosk completes provisioning automatically.',
  },
] as const;

export function KioskProvisioningCard({
  deviceName,
  registrationStatus,
}: KioskProvisioningCardProps) {
  const isRegistered = registrationStatus === 'registered';

  if (isRegistered) {
    return (
      <Alert severity="success" sx={{ mb: 3 }}>
        This kiosk is registered and ready for player sessions.
      </Alert>
    );
  }

  return (
    <Paper variant="outlined" sx={{ p: 3, mb: 3 }}>
      <Typography variant="h6" fontWeight={600} gutterBottom>
        Kiosk provisioning
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        {deviceName
          ? `Provision "${deviceName}" from the kiosk itself — no registration code is needed.`
          : 'Provision this device from the kiosk itself — no registration code is needed.'}
      </Typography>

      <Alert severity="info" sx={{ mb: 2 }}>
        Ensure device type, sub type, and location below match what you enter on the kiosk. An admin
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
