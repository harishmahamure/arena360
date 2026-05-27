import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  createExpense,
  type ExpenseCategory,
  getExpenseCategories,
} from '../../../services/expenses';
import { getVendors, type Vendor } from '../../../services/vendors';

export default function ExpenseNewPage() {
  const navigate = useNavigate();
  const [error, setError] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);
  const [categories, setCategories] = useState<ExpenseCategory[]>([]);
  const [vendors, setVendors] = useState<Vendor[]>([]);

  const [categoryId, setCategoryId] = useState('');
  const [vendorId, setVendorId] = useState('');
  const [amount, setAmount] = useState('');
  const [paymentMethod, setPaymentMethod] = useState('cash');
  const [expenseDate, setExpenseDate] = useState(new Date().toISOString().slice(0, 10));
  const [description, setDescription] = useState('');

  useEffect(() => {
    getExpenseCategories({ limit: 100 }).then((res) => setCategories(res.data));
    getVendors({ limit: 100 }).then((res) => setVendors(res.data));
  }, []);

  const handleSubmit = async () => {
    setError(undefined);
    if (!categoryId) {
      setError('Please select a category');
      return;
    }
    if (!amount || Number(amount) <= 0) {
      setError('Amount must be greater than 0');
      return;
    }

    setLoading(true);
    try {
      await createExpense({
        categoryId,
        vendorId: vendorId || undefined,
        amount: Number(amount),
        paymentMethod,
        expenseDate: new Date(expenseDate).toISOString(),
        description: description || undefined,
      });
      navigate('/expenses');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to create expense');
    } finally {
      setLoading(false);
    }
  };

  return (
    <Box sx={{ p: 3, maxWidth: 600 }}>
      <Typography variant="h4" fontWeight={600} gutterBottom>
        New Expense
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
        Record a new business expense
      </Typography>

      {error && (
        <Alert severity="error" sx={{ mb: 3 }} onClose={() => setError(undefined)}>
          {error}
        </Alert>
      )}

      <Card>
        <CardContent>
          <Stack spacing={3}>
            <TextField
              select
              label="Category"
              value={categoryId}
              onChange={(e) => setCategoryId(e.target.value)}
              fullWidth
              required
            >
              {categories.map((cat) => (
                <MenuItem key={cat.id} value={cat.id}>
                  {cat.name}
                </MenuItem>
              ))}
            </TextField>

            <TextField
              select
              label="Vendor (optional)"
              value={vendorId}
              onChange={(e) => setVendorId(e.target.value)}
              fullWidth
            >
              <MenuItem value="">None</MenuItem>
              {vendors.map((v) => (
                <MenuItem key={v.id} value={v.id}>
                  {v.name}
                </MenuItem>
              ))}
            </TextField>

            <TextField
              label="Amount"
              type="number"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              fullWidth
              required
              inputProps={{ min: 0, step: '0.01' }}
            />

            <TextField
              select
              label="Payment Method"
              value={paymentMethod}
              onChange={(e) => setPaymentMethod(e.target.value)}
              fullWidth
            >
              <MenuItem value="cash">Cash</MenuItem>
              <MenuItem value="online">Online</MenuItem>
              <MenuItem value="split_payment">Split Payment</MenuItem>
            </TextField>

            <TextField
              label="Expense Date"
              type="date"
              value={expenseDate}
              onChange={(e) => setExpenseDate(e.target.value)}
              fullWidth
              slotProps={{ inputLabel: { shrink: true } }}
            />

            <TextField
              label="Notes"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              fullWidth
              multiline
              rows={3}
            />

            <Stack direction="row" spacing={2}>
              <Button variant="contained" onClick={handleSubmit} disabled={loading} fullWidth>
                {loading ? 'Creating...' : 'Create Expense'}
              </Button>
              <Button
                variant="outlined"
                onClick={() => navigate('/expenses')}
                disabled={loading}
                fullWidth
              >
                Cancel
              </Button>
            </Stack>
          </Stack>
        </CardContent>
      </Card>
    </Box>
  );
}
