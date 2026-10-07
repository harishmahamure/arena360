import { CurrencyField, FormButton, FormPage } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import { Alert, Button, MenuItem, Stack, TextField } from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { GuidedForm, GuidedStep } from '../../../components/GuidedForm';
import { createExpense, getExpenseCategories } from '../../../services/expenses';
import { getVendors } from '../../../services/vendors';

export default function ExpenseNewPage() {
  const navigate = useNavigate();
  const [error, setError] = useState<string | undefined>();
  const {
    loading,
    succeeded,
    failed,
    errorMessage,
    disabled: actionDisabled,
    run,
  } = useAsyncAction({
    throttleMs: 1000,
    lockOnSuccess: true,
  });
  const categoriesQuery = useQuery({
    queryKey: ['expense-categories', 'expense-form'],
    queryFn: () => getExpenseCategories({ limit: 100 }),
  });
  const vendorsQuery = useQuery({
    queryKey: ['vendors', 'expense-form'],
    queryFn: () => getVendors({ limit: 100 }),
  });
  const categories = categoriesQuery.data?.data ?? [];
  const vendors = vendorsQuery.data?.data ?? [];

  const [categoryId, setCategoryId] = useState('');
  const [vendorId, setVendorId] = useState('');
  const [amount, setAmount] = useState('');
  const [paymentMethod, setPaymentMethod] = useState('cash');
  const [expenseDate, setExpenseDate] = useState(new Date().toISOString().slice(0, 10));
  const [description, setDescription] = useState('');

  const handleSubmit = () => {
    setError(undefined);
    if (!expenseDate || !Number.isFinite(new Date(expenseDate).getTime())) {
      setError('Choose a valid expense date');
      return;
    }
    if (!categoryId) {
      setError('Please select a category');
      return;
    }
    if (!amount || Number(amount) <= 0) {
      setError('Amount must be greater than 0');
      return;
    }

    void run(async () => {
      await createExpense({
        categoryId,
        vendorId: vendorId || undefined,
        amount: Number(amount),
        paymentMethod,
        expenseDate: new Date(expenseDate).toISOString(),
        description: description || undefined,
      });
      navigate('/expenses');
    });
  };

  return (
    <FormPage
      title="New Expense"
      description="Record a new business expense"
      backTo="/expenses"
      backLabel="Back to expenses"
      breadcrumbs={[{ label: 'Expenses', to: '/expenses' }, { label: 'New expense' }]}
      maxWidth={600}
    >
      {(categoriesQuery.isError || vendorsQuery.isError) && (
        <Alert
          severity="error"
          sx={{ mb: 3 }}
          action={
            <Button
              onClick={() => {
                void categoriesQuery.refetch();
                void vendorsQuery.refetch();
              }}
            >
              Retry
            </Button>
          }
        >
          Expense categories or vendors could not be loaded.
        </Alert>
      )}
      {error && (
        <Alert severity="error" sx={{ mb: 3 }} onClose={() => setError(undefined)}>
          {error}
        </Alert>
      )}

      <Stack spacing={3}>
        <GuidedForm
          busy={loading || succeeded}
          onCancel={() => navigate('/expenses')}
          actions={
            <>
              <Stack direction="row" spacing={2}>
                <FormButton
                  variant="contained"
                  onClick={handleSubmit}
                  loading={loading}
                  success={succeeded}
                  successLabel="Expense created"
                  error={failed}
                  errorLabel={errorMessage ?? 'Failed to create expense'}
                  disabled={actionDisabled || categoriesQuery.isLoading || categoriesQuery.isError}
                  fullWidth
                >
                  Create Expense
                </FormButton>
                <Button
                  data-wizard-cancel
                  variant="outlined"
                  onClick={() => navigate('/expenses')}
                  disabled={loading}
                  fullWidth
                >
                  Cancel
                </Button>
              </Stack>
            </>
          }
        >
          <GuidedStep
            title="Expense details"
            validate={() => (!categoryId ? 'Choose an expense category.' : undefined)}
          >
            <TextField
              select
              label="Category"
              disabled={categoriesQuery.isLoading || categoriesQuery.isError}
              value={categoryId}
              onChange={(e) => setCategoryId(e.target.value)}
              fullWidth
              required
              helperText="Expense category for reporting and budget tracking"
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
              disabled={vendorsQuery.isLoading || vendorsQuery.isError}
              value={vendorId}
              onChange={(e) => setVendorId(e.target.value)}
              fullWidth
              helperText="Link to a supplier if this expense was invoiced"
            >
              <MenuItem value="">None</MenuItem>
              {vendors.map((v) => (
                <MenuItem key={v.id} value={v.id}>
                  {v.name}
                </MenuItem>
              ))}
            </TextField>
          </GuidedStep>
          <GuidedStep
            title="Payment & date"
            validate={() =>
              !Number.isFinite(Number(amount)) || Number(amount) <= 0 || !expenseDate
                ? 'Enter a positive amount and a valid expense date.'
                : undefined
            }
          >
            <CurrencyField
              label="Amount"
              value={amount}
              onChange={(e) => setAmount(e.target.value)}
              fullWidth
              required
              helperText="Total expense amount in ₹ (must be greater than 0)"
            />
            <TextField
              select
              label="Payment Method"
              value={paymentMethod}
              onChange={(e) => setPaymentMethod(e.target.value)}
              fullWidth
              helperText="How the expense was paid out of the business"
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
              helperText="Date the expense was incurred (defaults to today)"
            />
          </GuidedStep>
          <GuidedStep title="Supporting notes">
            <TextField
              label="Notes"
              value={description}
              onChange={(e) => setDescription(e.target.value)}
              fullWidth
              multiline
              rows={3}
              helperText="Optional details such as invoice number or purpose"
            />
          </GuidedStep>
        </GuidedForm>
      </Stack>
    </FormPage>
  );
}
