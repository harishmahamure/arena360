import type { UserRole } from '@gaming-cafe/contracts';
import {
  CurrencyField,
  DetailPage,
  type DetailPageSection,
  type FieldConfig,
  FormBuilder,
  OtpField,
} from '@gaming-cafe/ui';
import { formatRemainingLabel, USERNAME_HELPER_TEXT } from '@gaming-cafe/utils';
import { ShoppingCart, Timer } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Divider,
  Grid,
  Link,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableContainer,
  TableHead,
  TableRow,
  TextField,
  Typography,
} from '@mui/material';
import { useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { Link as RouterLink, useNavigate, useParams } from 'react-router-dom';
import { SessionRemainingClock } from '../../../components/SessionRemainingClock';
import TotpQrCode from '../../../components/TotpQrCode';
import { PlayerActivePlansSection } from '../../../containers/players/PlayerActivePlansSection';
import { PlayerExhaustedPlansSection } from '../../../containers/players/PlayerExhaustedPlansSection';
import {
  adminCreateRoleOptions,
  type UpdatePlayerFormData,
  updatePlayerSchema,
  userRoleOptions,
} from '../../../containers/players/schemas/player-schema';
import { Permission, usePermissions } from '../../../hooks/usePermissions';
import { usePlayerProfile } from '../../../hooks/usePlayerProfile';
import { setCreditLimit } from '../../../services/players/setCreditLimit';
import { changePlayerPassword, updatePlayer } from '../../../services/players/update';
import { disableTotp, setupTotp, verifyTotpSetup } from '../../../services/users/totp';
import { formatDisplayDate, formatDisplayDateTime, formatDuration } from '../../../utils/date';

function formatCurrency(amount: number) {
  return new Intl.NumberFormat('en-IN', { style: 'currency', currency: 'INR' }).format(amount);
}

export default function PlayerDetailPage() {
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const { id } = useParams();
  const { can, isAdmin } = usePermissions();
  const canWrite = can(Permission.PlayersWrite);
  const canSetCreditLimit = can(Permission.CreditLimitWrite);
  const canBuyPlan = can(Permission.PlayerPlansWrite);
  const canStartSession = can(Permission.SessionsWrite);
  const roleOptions = isAdmin ? userRoleOptions : adminCreateRoleOptions;

  const {
    player,
    isMember,
    canReadPlans,
    canReadCredit,
    canReadSessions,
    activePlans,
    exhaustedPlans,
    primaryActivePlan,
    activeSession,
    recentSessions,
    credit,
    isLoading,
    error,
    refetchPlayer,
  } = usePlayerProfile(id);

  const editPlayerFormFields = useMemo<FieldConfig<UpdatePlayerFormData>[]>(
    () => [
      {
        name: 'username',
        label: 'Username',
        type: 'username',
        placeholder: 'e.g., johndoe',
        required: true,
        gridCols: 6,
        helperText: USERNAME_HELPER_TEXT,
      },
      {
        name: 'phoneNumber',
        label: 'Phone Number',
        type: 'phone',
        placeholder: 'e.g., 9876543210',
        required: true,
        gridCols: 6,
        helperText: 'Required, minimum 10 digits',
      },
      {
        name: 'firstName',
        label: 'First Name',
        type: 'text',
        placeholder: 'e.g., John',
        gridCols: 6,
        helperText: 'Optional (max 50 characters)',
      },
      {
        name: 'lastName',
        label: 'Last Name',
        type: 'text',
        placeholder: 'e.g., Doe',
        gridCols: 6,
        helperText: 'Optional (max 50 characters)',
      },
      {
        name: 'role',
        label: 'Role',
        type: 'select',
        gridCols: 6,
        options: roleOptions,
        helperText: 'User role in the system',
      },
      {
        name: 'isActive',
        label: 'Account Active',
        type: 'switch',
        gridCols: 6,
        helperText: 'Toggle to activate/deactivate the account',
      },
    ],
    [roleOptions],
  );

  const [formError, setFormError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [totpSecret, setTotpSecret] = useState('');
  const [totpUri, setTotpUri] = useState('');
  const [totpCode, setTotpCode] = useState('');
  const [totpEnabled, setTotpEnabled] = useState(false);
  const [totpLoading, setTotpLoading] = useState(false);
  const [newPassword, setNewPassword] = useState('');
  const [passwordLoading, setPasswordLoading] = useState(false);
  const [creditLimitInput, setCreditLimitInput] = useState('');
  const [creditLimitLoading, setCreditLimitLoading] = useState(false);

  useEffect(() => {
    if (player?.totpEnabled !== undefined) {
      setTotpEnabled(Boolean(player.totpEnabled));
    }
  }, [player?.totpEnabled]);

  useEffect(() => {
    if (player?.creditLimit !== undefined) {
      setCreditLimitInput(String(player.creditLimit));
    }
  }, [player?.creditLimit]);

  const displayName =
    player?.firstName && player?.lastName
      ? `${player.firstName} ${player.lastName}`
      : player?.username;

  const handleSubmit = async (data: UpdatePlayerFormData) => {
    setIsSubmitting(true);
    setFormError(undefined);
    setSuccess(undefined);

    if (!data.username || !data.phoneNumber) {
      setFormError('Username and phone number are required');
      setIsSubmitting(false);
      return;
    }

    try {
      await updatePlayer(id as string, {
        username: data.username,
        phoneNumber: data.phoneNumber,
        firstName: data.firstName || undefined,
        lastName: data.lastName || undefined,
        role: data.role as UserRole,
        isActive: data.isActive,
      });

      setSuccess('Player updated successfully!');
      await queryClient.invalidateQueries({ queryKey: ['player', id] });
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Failed to update player');
    } finally {
      setIsSubmitting(false);
    }
  };

  const handleSetupTotp = async () => {
    if (!id) return;
    setTotpLoading(true);
    try {
      const response = await setupTotp(id);
      setTotpSecret(response.secret);
      setTotpUri(response.otpauthUri);
      setTotpEnabled(response.totpEnabled);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Failed to setup TOTP');
    } finally {
      setTotpLoading(false);
    }
  };

  const handleVerifyTotp = async () => {
    if (!id) return;
    setTotpLoading(true);
    try {
      const response = await verifyTotpSetup(id, totpCode);
      setTotpEnabled(response.totpEnabled);
      setTotpCode('');
      setSuccess('TOTP enabled successfully');
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Failed to verify TOTP');
    } finally {
      setTotpLoading(false);
    }
  };

  const handleDisableTotp = async () => {
    if (!id) return;
    setTotpLoading(true);
    try {
      await disableTotp(id);
      setTotpEnabled(false);
      setTotpSecret('');
      setTotpUri('');
      setSuccess('TOTP disabled');
    } catch (err) {
      setFormError(err instanceof Error ? err.message : 'Failed to disable TOTP');
    } finally {
      setTotpLoading(false);
    }
  };

  const summary = useMemo(() => {
    if (!player) return undefined;

    return (
      <Grid container spacing={2} alignItems="center">
        <Grid size={{ xs: 12, md: 6 }}>
          <Typography variant="h5" fontWeight={600}>
            {displayName}
          </Typography>
          <Typography variant="subtitle1" color="text.secondary">
            @{player.username}
          </Typography>
          {player.phoneNumber && (
            <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
              {player.phoneNumber}
            </Typography>
          )}
          <Typography variant="body2" color="text.secondary">
            Member since {formatDisplayDate(player.createdAt)}
          </Typography>
          {player.role !== 'player' && (
            <Chip label={player.role} size="small" color="warning" sx={{ mt: 1 }} />
          )}
        </Grid>
        {isMember && canReadPlans && (
          <Grid size={{ xs: 12, md: 6 }}>
            {activeSession ? (
              <Card variant="outlined">
                <CardContent>
                  <Typography variant="caption" color="text.secondary" display="block">
                    Active session
                  </Typography>
                  <Link
                    component={RouterLink}
                    to={`/sessions/${activeSession.id}`}
                    underline="hover"
                  >
                    <Typography variant="subtitle1" fontWeight={600}>
                      {activeSession.device?.name ?? 'View session'}
                    </Typography>
                  </Link>
                  {activeSession.balance?.remainingMinutes != null && (
                    <Box sx={{ mt: 1 }}>
                      <SessionRemainingClock
                        variant="prominent"
                        sessionStartTime={activeSession.startTime}
                        remainingMinutes={activeSession.balance.remainingMinutes}
                        timeCreditsConsumed={activeSession.timeCreditsConsumed}
                        deductionProfile={activeSession.balance.deductionProfile}
                        expiryDate={activeSession.balance.expiryDate}
                      />
                    </Box>
                  )}
                </CardContent>
              </Card>
            ) : primaryActivePlan ? (
              <Card variant="outlined">
                <CardContent>
                  <Typography variant="caption" color="text.secondary" display="block">
                    Soonest expiring plan
                  </Typography>
                  <Typography variant="subtitle1" fontWeight={600}>
                    {primaryActivePlan.plan?.name ?? 'Active plan'}
                  </Typography>
                  <Typography variant="body2" sx={{ mt: 0.5 }}>
                    {formatRemainingLabel(primaryActivePlan.remainingMinutes)} remaining
                  </Typography>
                  <Typography variant="body2" color="text.secondary">
                    Expires {formatDisplayDate(primaryActivePlan.expiryDate)}
                  </Typography>
                </CardContent>
              </Card>
            ) : (
              <Alert severity="info">
                No active plans.
                {canBuyPlan && id && (
                  <>
                    {' '}
                    <Link component={RouterLink} to={`/plan-transactions/new?playerId=${id}`}>
                      Buy a plan
                    </Link>
                  </>
                )}
              </Alert>
            )}
          </Grid>
        )}
      </Grid>
    );
  }, [
    player,
    displayName,
    isMember,
    canReadPlans,
    activeSession,
    primaryActivePlan,
    canBuyPlan,
    id,
  ]);

  const sections: DetailPageSection[] = useMemo(() => {
    if (!player) return [];

    const result: DetailPageSection[] = [];

    if (isMember && canReadPlans) {
      result.push({
        title: 'Active plans',
        description: 'Wallet balances available for kiosk login and sessions.',
        content: <PlayerActivePlansSection plans={activePlans} />,
      });

      if (exhaustedPlans.length > 0) {
        result.push({
          title: 'Recently exhausted',
          description: 'Last 2 plans that ran out of minutes.',
          content: <PlayerExhaustedPlansSection plans={exhaustedPlans} />,
        });
      }
    }

    if (isMember && canReadCredit && credit) {
      result.push({
        title: 'Credit / Khata',
        content: (
          <Card variant="outlined">
            <CardContent>
              <Grid container spacing={2}>
                <Grid size={{ xs: 12, sm: 4 }}>
                  <Typography variant="caption" color="text.secondary">
                    Outstanding
                  </Typography>
                  <Typography variant="h6">{formatCurrency(credit.summary.outstanding)}</Typography>
                </Grid>
                <Grid size={{ xs: 12, sm: 4 }}>
                  <Typography variant="caption" color="text.secondary">
                    Credit limit
                  </Typography>
                  <Typography variant="h6">{formatCurrency(credit.summary.creditLimit)}</Typography>
                </Grid>
                <Grid size={{ xs: 12, sm: 4 }}>
                  <Typography variant="caption" color="text.secondary">
                    Available
                  </Typography>
                  <Typography variant="h6">{formatCurrency(credit.summary.available)}</Typography>
                </Grid>
              </Grid>
              <Button component={RouterLink} to="/credit" size="small" sx={{ mt: 2 }}>
                Manage running tab
              </Button>
            </CardContent>
          </Card>
        ),
      });
    }

    if (isMember && canReadSessions && recentSessions.length > 0) {
      result.push({
        title: 'Recent sessions',
        content: (
          <TableContainer component={Paper} variant="outlined">
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>Device</TableCell>
                  <TableCell>Started</TableCell>
                  <TableCell>Duration</TableCell>
                  <TableCell>Status</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {recentSessions.map((session) => (
                  <TableRow key={session.id} hover>
                    <TableCell>
                      <Link component={RouterLink} to={`/sessions/${session.id}`} underline="hover">
                        {session.device?.name ?? 'Session'}
                      </Link>
                    </TableCell>
                    <TableCell>{formatDisplayDateTime(session.startTime)}</TableCell>
                    <TableCell>
                      {session.durationMinutes != null
                        ? formatDuration(session.durationMinutes)
                        : session.endTime
                          ? '—'
                          : 'In progress'}
                    </TableCell>
                    <TableCell>
                      <Chip
                        label={session.endTime ? 'Ended' : 'Active'}
                        size="small"
                        color={session.endTime ? 'default' : 'success'}
                      />
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          </TableContainer>
        ),
      });
    }

    return result;
  }, [
    player,
    isMember,
    canReadPlans,
    canReadCredit,
    canReadSessions,
    activePlans,
    exhaustedPlans,
    credit,
    recentSessions,
  ]);

  const profileActions =
    isMember && (canBuyPlan || canStartSession) ? (
      <Stack direction="row" spacing={1} flexWrap="wrap" useFlexGap>
        {canBuyPlan && id && (
          <Button
            variant="contained"
            startIcon={<ShoppingCart />}
            component={RouterLink}
            to={`/plan-transactions/new?playerId=${id}`}
          >
            Buy plan
          </Button>
        )}
        {canStartSession && id && (
          <Button
            variant="outlined"
            startIcon={<Timer />}
            component={RouterLink}
            to={`/sessions/new?playerId=${id}`}
          >
            Start session
          </Button>
        )}
      </Stack>
    ) : undefined;

  return (
    <>
      <DetailPage
        title={displayName ?? 'Player'}
        description="Member profile, plans, and account settings"
        backTo="/players"
        backLabel="Back to players"
        breadcrumbs={[{ label: 'Players', to: '/players' }, { label: displayName ?? 'Player' }]}
        isLoading={isLoading}
        error={
          !isLoading && (error || !player)
            ? error instanceof Error
              ? error.message
              : 'Player not found'
            : null
        }
        onRetry={() => void refetchPlayer()}
        status={{
          label: player?.isActive ? 'Active' : 'Inactive',
          color: player?.isActive ? 'success' : 'default',
        }}
        banner={
          success ? (
            <Alert severity="success" sx={{ mb: 2 }} onClose={() => setSuccess(undefined)}>
              {success}
            </Alert>
          ) : undefined
        }
        summary={summary}
        sections={sections}
        actions={profileActions}
      />

      <Paper elevation={0} sx={{ p: 4, mt: 3 }}>
        <Box sx={{ mb: 3 }}>
          <Typography variant="h5" fontWeight={600} gutterBottom>
            Account settings
          </Typography>
          <Typography variant="body2" color="text.secondary">
            Update login details and security for {displayName}
          </Typography>
        </Box>

        <FormBuilder<UpdatePlayerFormData>
          fields={editPlayerFormFields}
          schema={updatePlayerSchema}
          defaultValues={{
            username: player?.username || '',
            phoneNumber: player?.phoneNumber || '',
            firstName: player?.firstName || '',
            lastName: player?.lastName || '',
            role: player?.role || 'player',
            isActive: player?.isActive ?? true,
          }}
          mode={canWrite ? 'edit' : 'view'}
          onSubmit={handleSubmit}
          onCancel={() => navigate('/players')}
          loading={isSubmitting}
          error={formError}
          success={undefined}
          showCancel={canWrite}
          showReset={canWrite}
          submitLabel="Update Player"
          cancelLabel="Back to list"
          buttonAlign="right"
          spacing={3}
        />

        {canSetCreditLimit && player?.role === 'player' && (
          <Box sx={{ mt: 4 }}>
            <Divider sx={{ mb: 3 }} />
            <Typography variant="h6" fontWeight={600} gutterBottom>
              Credit Limit (Tab / Khata)
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              Set to 0 to disable credit purchases for this member.
            </Typography>
            <Box sx={{ display: 'flex', gap: 2, maxWidth: 480, alignItems: 'flex-start' }}>
              <CurrencyField
                label="Credit Limit (INR)"
                size="small"
                value={creditLimitInput}
                onChange={(e) => setCreditLimitInput(e.target.value)}
                helperText="Maximum outstanding credit allowed"
                sx={{ flex: 1 }}
              />
              <Button
                variant="contained"
                disabled={creditLimitLoading || creditLimitInput === ''}
                onClick={async () => {
                  if (!id) return;
                  setCreditLimitLoading(true);
                  try {
                    await setCreditLimit(id, Number.parseFloat(creditLimitInput));
                    setSuccess('Credit limit updated');
                    await queryClient.invalidateQueries({ queryKey: ['player', id] });
                  } catch (err) {
                    setFormError(
                      err instanceof Error ? err.message : 'Failed to update credit limit',
                    );
                  } finally {
                    setCreditLimitLoading(false);
                  }
                }}
              >
                Save
              </Button>
            </Box>
          </Box>
        )}

        {canWrite && player?.role !== 'admin' && (
          <Box sx={{ mt: 4 }}>
            <Divider sx={{ mb: 3 }} />
            <Typography variant="h6" fontWeight={600} gutterBottom>
              Change Password
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              Set a new password for this user.
            </Typography>
            <Box sx={{ display: 'flex', gap: 2, maxWidth: 480, alignItems: 'flex-start' }}>
              <TextField
                label="New Password"
                type="password"
                size="small"
                value={newPassword}
                onChange={(e) => setNewPassword(e.target.value)}
                helperText="Minimum 8 characters"
                sx={{ flex: 1 }}
              />
              <Button
                variant="contained"
                disabled={passwordLoading || newPassword.length < 8}
                onClick={async () => {
                  if (!id) return;
                  setPasswordLoading(true);
                  try {
                    await changePlayerPassword(id, newPassword);
                    setNewPassword('');
                    setSuccess('Password changed successfully');
                  } catch (err) {
                    setFormError(err instanceof Error ? err.message : 'Failed to change password');
                  } finally {
                    setPasswordLoading(false);
                  }
                }}
              >
                Change
              </Button>
            </Box>
          </Box>
        )}

        {isAdmin && (player?.role === 'staff' || player?.role === 'admin') && (
          <Box sx={{ mt: 4 }}>
            <Divider sx={{ mb: 3 }} />
            <Typography variant="h6" fontWeight={600} gutterBottom>
              {player?.role === 'admin' ? 'Admin TOTP Setup' : 'Staff TOTP Setup'}
            </Typography>
            <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
              {player?.role === 'admin'
                ? 'Configure authenticator TOTP for admin panel and kiosk sign-in.'
                : 'Configure authenticator TOTP for shift handover validation.'}
            </Typography>
            <Typography variant="body2" sx={{ mb: 2 }}>
              Status: {totpEnabled ? 'Enabled' : 'Not enabled'}
            </Typography>
            {!totpSecret && !totpEnabled && (
              <Button variant="outlined" onClick={handleSetupTotp} disabled={totpLoading}>
                Generate TOTP Secret
              </Button>
            )}
            {totpSecret && !totpEnabled && (
              <Box sx={{ display: 'grid', gap: 2, maxWidth: 480 }}>
                {totpUri && (
                  <TotpQrCode value={totpUri} accountLabel={player?.username} secret={totpSecret} />
                )}
                <OtpField
                  label="Verification Code"
                  value={totpCode}
                  onChange={(event) => setTotpCode(event.target.value)}
                />
                <Box sx={{ display: 'flex', gap: 1 }}>
                  <Button variant="contained" onClick={handleVerifyTotp} disabled={totpLoading}>
                    Verify & Enable
                  </Button>
                </Box>
              </Box>
            )}
            {totpEnabled && (
              <Button
                color="error"
                variant="outlined"
                onClick={handleDisableTotp}
                disabled={totpLoading}
              >
                Disable TOTP
              </Button>
            )}
          </Box>
        )}
      </Paper>
    </>
  );
}
