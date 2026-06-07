import { toastUtils } from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import {
  approveExpense,
  type Expense,
  getExpense,
  rejectExpense,
  updateExpense,
} from '../../../services/expenses';
import { formatDisplayDate } from '../../../utils/date';

export default function ExpenseDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const [rejectDialogOpen, setRejectDialogOpen] = useState(false);
  const [rejectionReason, setRejectionReason] = useState('');
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | undefined>();

  const [editAmount, setEditAmount] = useState('');
  const [editPaymentMethod, setEditPaymentMethod] = useState('');
  const [editDescription, setEditDescription] = useState('');

  const { data: expense, isLoading } = useQuery({
    queryKey: ['expense', id],
    queryFn: () => getExpense(id as string),
    enabled: !!id,
  });

  const populateForm = (exp: Expense) => {
    if (!editAmount) {
      setEditAmount(String(exp.amount));
      setEditPaymentMethod(exp.paymentMethod);
      setEditDescription(exp.description || '');
    }
  };

  if (expense && !editAmount) {
    populateForm(expense);
  }

  const isPending = expense?.approvalStatus === 'pending';

  const handleSave = async () => {
    if (!id) return;
    setSaving(true);
    setError(undefined);
    try {
      await updateExpense(id, {
        amount: Number(editAmount),
        paymentMethod: editPaymentMethod,
        description: editDescription || undefined,
      });
      toastUtils.success('Expense updated');
      queryClient.invalidateQueries({ queryKey: ['expense', id] });
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to update');
    } finally {
      setSaving(false);
    }
  };

  const handleApprove = async () => {
    if (!id) return;
    setSaving(true);
    try {
      await approveExpense(id);
      toastUtils.success('Expense approved');
      queryClient.invalidateQueries({ queryKey: ['expense', id] });
    } catch {
      toastUtils.error('Failed to approve expense');
    } finally {
      setSaving(false);
    }
  };

  const handleRejectConfirm = async () => {
    if (!id || !rejectionReason.trim()) return;
    setSaving(true);
    try {
      await rejectExpense(id, rejectionReason.trim());
      toastUtils.success('Expense rejected');
      setRejectDialogOpen(false);
      queryClient.invalidateQueries({ queryKey: ['expense', id] });
    } catch {
      toastUtils.error('Failed to reject expense');
    } finally {
      setSaving(false);
    }
  };

  if (isLoading) {
    return (
      <Box sx={{ p: 3 }}>
        <Typography>Loading...</Typography>
      </Box>
    );
  }

  if (!expense) {
    return (
      <Box sx={{ p: 3 }}>
        <Alert severity="error">Expense not found</Alert>
      </Box>
    );
  }

  const statusColor =
    expense.approvalStatus === 'approved'
      ? 'success'
      : expense.approvalStatus === 'rejected'
        ? 'error'
        : 'warning';

  return (
    <Box sx={{ p: 3, maxWidth: 600 }}>
      <Box sx={{ display: 'flex', alignItems: 'center', gap: 2, mb: 3 }}>
        <Typography variant="h4" fontWeight={600}>
          Expense Details
        </Typography>
        <Chip
          label={expense.approvalStatus}
          color={statusColor as 'success' | 'error' | 'warning'}
          size="small"
        />
      </Box>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }} onClose={() => setError(undefined)}>
          {error}
        </Alert>
      )}

      <Card>
        <CardContent>
          <Stack spacing={3}>
            {isPending ? (
              <>
                <TextField
                  label="Amount"
                  type="number"
                  value={editAmount}
                  onChange={(e) => setEditAmount(e.target.value)}
                  fullWidth
                  inputProps={{ min: 0, step: '0.01' }}
                />
                <TextField
                  select
                  label="Payment Method"
                  value={editPaymentMethod}
                  onChange={(e) => setEditPaymentMethod(e.target.value)}
                  fullWidth
                >
                  <MenuItem value="cash">Cash</MenuItem>
                  <MenuItem value="online">Online</MenuItem>
                  <MenuItem value="split_payment">Split Payment</MenuItem>
                </TextField>
                <TextField
                  label="Notes"
                  value={editDescription}
                  onChange={(e) => setEditDescription(e.target.value)}
                  fullWidth
                  multiline
                  rows={3}
                />
              </>
            ) : (
              <>
                <Box>
                  <Typography variant="caption" color="text.secondary">
                    Amount
                  </Typography>
                  <Typography variant="h6">
                    {new Intl.NumberFormat('en-IN', {
                      style: 'currency',
                      currency: 'INR',
                    }).format(expense.amount)}
                  </Typography>
                </Box>
                <Box>
                  <Typography variant="caption" color="text.secondary">
                    Payment Method
                  </Typography>
                  <Typography>{expense.paymentMethod}</Typography>
                </Box>
                <Box>
                  <Typography variant="caption" color="text.secondary">
                    Description
                  </Typography>
                  <Typography>{expense.description || '-'}</Typography>
                </Box>
                {expense.rejectionReason && (
                  <Box>
                    <Typography variant="caption" color="text.secondary">
                      Rejection Reason
                    </Typography>
                    <Typography color="error">{expense.rejectionReason}</Typography>
                  </Box>
                )}
              </>
            )}

            <Box>
              <Typography variant="caption" color="text.secondary">
                Expense Date
              </Typography>
              <Typography>{formatDisplayDate(expense.expenseDate)}</Typography>
            </Box>

            <Box>
              <Typography variant="caption" color="text.secondary">
                Created At
              </Typography>
              <Typography>{formatDisplayDate(expense.createdAt)}</Typography>
            </Box>

            <Stack direction="row" spacing={2}>
              {isPending && (
                <>
                  <Button variant="contained" onClick={handleSave} disabled={saving}>
                    Save
                  </Button>
                  <Button
                    variant="contained"
                    color="success"
                    onClick={handleApprove}
                    disabled={saving}
                  >
                    Approve
                  </Button>
                  <Button
                    variant="contained"
                    color="error"
                    onClick={() => setRejectDialogOpen(true)}
                    disabled={saving}
                  >
                    Reject
                  </Button>
                </>
              )}
              <Button variant="outlined" onClick={() => navigate('/expenses')}>
                Back
              </Button>
            </Stack>
          </Stack>
        </CardContent>
      </Card>

      <Dialog open={rejectDialogOpen} onClose={() => setRejectDialogOpen(false)} fullWidth>
        <DialogTitle>Reject Expense</DialogTitle>
        <DialogContent>
          <TextField
            label="Rejection Reason"
            value={rejectionReason}
            onChange={(e) => setRejectionReason(e.target.value)}
            fullWidth
            multiline
            rows={3}
            sx={{ mt: 1 }}
            required
          />
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setRejectDialogOpen(false)}>Cancel</Button>
          <Button
            color="error"
            variant="contained"
            onClick={handleRejectConfirm}
            disabled={!rejectionReason.trim() || saving}
          >
            Reject
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
