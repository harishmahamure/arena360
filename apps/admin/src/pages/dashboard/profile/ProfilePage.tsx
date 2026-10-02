import { toastUtils } from '@gaming-cafe/utils';
import { PhotoCamera } from '@mui/icons-material';
import {
  Avatar,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Divider,
  Stack,
  Typography,
} from '@mui/material';
import { useMemo, useRef, useState } from 'react';
import { useDispatch, useSelector } from '../../../hooks/store';
import { sessionPermissions } from '../../../lib/authSession';
import { useBranding } from '../../../services/config/branding';
import { uploadAsset } from '../../../services/upload/presign';
import { updateOwnAvatar } from '../../../services/users/avatar';

function titleCase(value: string): string {
  return value.replace(/[-_]/g, ' ').replace(/\b\w/g, (letter) => letter.toUpperCase());
}

export function groupPermissions(permissions: string[]): [string, string[]][] {
  const groups = new Map<string, string[]>();
  for (const permission of permissions) {
    const [module = permission, action = 'access'] = permission.split(':');
    groups.set(module, [...(groups.get(module) ?? []), action]);
  }
  return [...groups.entries()].sort(([a], [b]) => a.localeCompare(b));
}

export default function ProfilePage() {
  const auth = useSelector((state) => state.auth);
  const dispatch = useDispatch();
  const brand = useBranding();
  const fileInput = useRef<HTMLInputElement>(null);
  const [saving, setSaving] = useState(false);
  const fullName = `${auth.firstName} ${auth.lastName}`.trim() || auth.username || 'Account';
  const access = useMemo(() => groupPermissions(sessionPermissions()), []);

  const savePhoto = async (file: File | null) => {
    setSaving(true);
    try {
      const url = file ? await uploadAsset(file, { purpose: 'avatar', maxDimension: 512 }) : null;
      const user = await updateOwnAvatar(url);
      dispatch({ type: 'SetAuthDetail', payload: { ...auth, avatarUrl: user.avatarUrl ?? '' } });
      toastUtils.success(file ? 'Profile photo updated' : 'Profile photo removed');
    } catch (error) {
      toastUtils.error(error instanceof Error ? error.message : 'Could not update your photo');
    } finally {
      setSaving(false);
      if (fileInput.current) fileInput.current.value = '';
    }
  };

  return (
    <Stack spacing={3} sx={{ p: { xs: 2, md: 4 }, maxWidth: 960 }}>
      <Card variant="outlined">
        <CardContent>
          <Stack direction={{ xs: 'column', sm: 'row' }} spacing={3} alignItems="center">
            <Stack spacing={1} alignItems="center">
              <Avatar
                src={auth.avatarUrl || undefined}
                alt={fullName}
                sx={{ width: 96, height: 96, fontSize: '2rem', bgcolor: 'primary.main' }}
              >
                {fullName.slice(0, 1).toUpperCase()}
              </Avatar>
              <input
                ref={fileInput}
                type="file"
                accept="image/*"
                hidden
                onChange={(event) => void savePhoto(event.target.files?.[0] ?? null)}
              />
              <Stack direction="row" spacing={1}>
                <Button
                  size="small"
                  variant="outlined"
                  startIcon={saving ? <CircularProgress size={14} /> : <PhotoCamera />}
                  disabled={saving}
                  onClick={() => fileInput.current?.click()}
                >
                  {auth.avatarUrl ? 'Change photo' : 'Add photo'}
                </Button>
                {auth.avatarUrl && (
                  <Button
                    size="small"
                    color="error"
                    disabled={saving}
                    onClick={() => void savePhoto(null)}
                  >
                    Remove
                  </Button>
                )}
              </Stack>
            </Stack>
            <Box sx={{ minWidth: 0, textAlign: { xs: 'center', sm: 'left' } }}>
              <Typography variant="h5" fontWeight={700} noWrap>
                {fullName}
              </Typography>
              <Typography color="text.secondary">@{auth.username}</Typography>
              <Stack
                direction="row"
                spacing={1}
                sx={{ mt: 1, justifyContent: { xs: 'center', sm: 'flex-start' } }}
              >
                <Chip label={titleCase(auth.role || 'staff')} size="small" color="primary" />
                <Chip label={brand.name} size="small" variant="outlined" />
              </Stack>
            </Box>
          </Stack>
        </CardContent>
      </Card>

      <Card variant="outlined">
        <CardContent>
          <Typography variant="h6" gutterBottom>
            Account
          </Typography>
          <Box
            component="dl"
            sx={{ display: 'grid', gridTemplateColumns: 'max-content 1fr', gap: 1.5, m: 0 }}
          >
            {[
              ['Username', auth.username],
              ['Email', auth.email || 'Not set'],
              ['Role', titleCase(auth.role || 'staff')],
            ].map(([label, value]) => (
              <Box key={label} sx={{ display: 'contents' }}>
                <Typography component="dt" color="text.secondary">
                  {label}
                </Typography>
                <Typography component="dd" sx={{ m: 0 }}>
                  {value}
                </Typography>
              </Box>
            ))}
          </Box>
          <Divider sx={{ my: 2 }} />
          <Typography variant="body2" color="text.secondary">
            Your sign-in renews automatically while you work. You are signed out after a period of
            inactivity. Photos are converted to WebP before upload. Ask an administrator to change
            your name, email or role.
          </Typography>
        </CardContent>
      </Card>

      <Card variant="outlined">
        <CardContent>
          <Typography variant="h6" gutterBottom>
            What you can access
          </Typography>
          {access.length === 0 ? (
            <Typography color="text.secondary">No permissions are assigned to you yet.</Typography>
          ) : (
            <Stack divider={<Divider flexItem />} spacing={1.5}>
              {access.map(([module, actions]) => (
                <Stack
                  key={module}
                  direction={{ xs: 'column', sm: 'row' }}
                  spacing={1}
                  alignItems={{ sm: 'center' }}
                >
                  <Typography fontWeight={600} sx={{ minWidth: 180 }}>
                    {titleCase(module)}
                  </Typography>
                  <Stack direction="row" spacing={0.75} useFlexGap flexWrap="wrap">
                    {actions.map((action) => (
                      <Chip
                        key={action}
                        label={titleCase(action)}
                        size="small"
                        variant="outlined"
                      />
                    ))}
                  </Stack>
                </Stack>
              ))}
            </Stack>
          )}
        </CardContent>
      </Card>
    </Stack>
  );
}
