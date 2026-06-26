import type { DeviceStatusValue } from '@gaming-cafe/contracts';
import { DeviceStatus } from '@gaming-cafe/contracts';
import { Build, CheckCircle, Error as ErrorIcon, PlayArrow } from '@mui/icons-material';

export function deviceStatusColor(
  status: DeviceStatusValue,
): 'default' | 'primary' | 'success' | 'warning' | 'error' {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
    case DeviceStatus.AVAILABLE:
      return 'success';
    case DeviceStatus.UNDER_MAINTENANCE:
      return 'warning';
    case DeviceStatus.OUT_OF_SERVICE:
      return 'error';
    case DeviceStatus.IN_USE:
      return 'primary';
    default:
      return 'default';
  }
}

export function deviceStatusAccentColor(status: DeviceStatusValue): string {
  switch (status) {
    case DeviceStatus.IN_USE:
      return 'primary.main';
    case DeviceStatus.AVAILABLE:
    case DeviceStatus.OPERATIONAL:
      return 'success.main';
    case DeviceStatus.UNDER_MAINTENANCE:
      return 'warning.main';
    case DeviceStatus.OUT_OF_SERVICE:
      return 'error.main';
    default:
      return 'divider';
  }
}

export function deviceStatusIcon(status: DeviceStatusValue) {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
    case DeviceStatus.AVAILABLE:
      return <CheckCircle fontSize="small" />;
    case DeviceStatus.UNDER_MAINTENANCE:
      return <Build fontSize="small" />;
    case DeviceStatus.OUT_OF_SERVICE:
      return <ErrorIcon fontSize="small" />;
    case DeviceStatus.IN_USE:
      return <PlayArrow fontSize="small" />;
    default:
      return null;
  }
}

export function deviceStatusLabel(status: DeviceStatusValue): string {
  switch (status) {
    case DeviceStatus.OPERATIONAL:
      return 'Ready';
    case DeviceStatus.UNDER_MAINTENANCE:
      return 'Maintenance';
    case DeviceStatus.OUT_OF_SERVICE:
      return 'Out of order';
    case DeviceStatus.IN_USE:
      return 'In use';
    case DeviceStatus.AVAILABLE:
      return 'Available';
    default:
      return status;
  }
}

/** Sort: active sessions first, then idle stations, then offline/maintenance. */
export function deviceStatusSortRank(status: DeviceStatusValue): number {
  switch (status) {
    case DeviceStatus.IN_USE:
      return 0;
    case DeviceStatus.AVAILABLE:
    case DeviceStatus.OPERATIONAL:
      return 1;
    case DeviceStatus.UNDER_MAINTENANCE:
      return 2;
    case DeviceStatus.OUT_OF_SERVICE:
      return 3;
    default:
      return 4;
  }
}
