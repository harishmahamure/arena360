import { FormButton, FormPage, PhoneField } from '@gaming-cafe/ui';
import { digitsOnly, trimValue, useAsyncAction } from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Button,
  FormControlLabel,
  FormHelperText,
  Stack,
  Switch,
  TextField,
} from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { createVendor } from '../../../services/vendors';

export default function VendorNewPage() {
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

  const [name, setName] = useState('');
  const [contactPerson, setContactPerson] = useState('');
  const [phone, setPhone] = useState('');
  const [email, setEmail] = useState('');
  const [address, setAddress] = useState('');
  const [gstNumber, setGstNumber] = useState('');
  const [isActive, setIsActive] = useState(true);

  const handleSubmit = () => {
    setError(undefined);
    if (!name.trim()) {
      setError('Name is required');
      return;
    }

    void run(async () => {
      await createVendor({
        name: trimValue(name),
        contactPerson: trimValue(contactPerson) || undefined,
        phone: digitsOnly(phone) || undefined,
        email: trimValue(email) || undefined,
        address: trimValue(address) || undefined,
        gstNumber: trimValue(gstNumber) || undefined,
        isActive,
      });
      navigate('/vendors');
    });
  };

  return (
    <FormPage
      title="New Vendor"
      description="Add a new supplier or vendor"
      backTo="/vendors"
      backLabel="Back to vendors"
      breadcrumbs={[{ label: 'Vendors', to: '/vendors' }, { label: 'New vendor' }]}
      maxWidth={600}
    >
      {error && (
        <Alert severity="error" sx={{ mb: 3 }} onClose={() => setError(undefined)}>
          {error}
        </Alert>
      )}

      <Stack spacing={3}>
        <TextField
          label="Name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          fullWidth
          required
          helperText="Business or supplier name shown on expense records"
        />
        <TextField
          label="Contact Person"
          value={contactPerson}
          onChange={(e) => setContactPerson(e.target.value)}
          fullWidth
          helperText="Primary contact at the vendor (optional)"
        />
        <PhoneField
          label="Phone"
          value={phone}
          onChange={(e) => setPhone(e.target.value)}
          fullWidth
          helperText="Contact phone number (optional)"
        />
        <TextField
          label="Email"
          type="email"
          value={email}
          onChange={(e) => setEmail(e.target.value)}
          fullWidth
          helperText="Contact email for invoices and orders (optional)"
        />
        <TextField
          label="Address"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          fullWidth
          multiline
          rows={2}
          helperText="Vendor address for records (optional)"
        />
        <TextField
          label="GST Number"
          value={gstNumber}
          onChange={(e) => setGstNumber(e.target.value)}
          fullWidth
          helperText="GSTIN for tax reporting (optional)"
        />
        <Box>
          <FormControlLabel
            control={<Switch checked={isActive} onChange={(e) => setIsActive(e.target.checked)} />}
            label="Active"
          />
          <FormHelperText sx={{ mx: 0 }}>
            Inactive vendors are hidden from expense vendor pickers
          </FormHelperText>
        </Box>

        <Stack direction="row" spacing={2}>
          <FormButton
            variant="contained"
            onClick={handleSubmit}
            loading={loading}
            success={succeeded}
            successLabel="Vendor created"
            error={failed}
            errorLabel={errorMessage ?? 'Failed to create vendor'}
            disabled={actionDisabled}
            fullWidth
          >
            Create Vendor
          </FormButton>
          <Button
            variant="outlined"
            onClick={() => navigate('/vendors')}
            disabled={loading}
            fullWidth
          >
            Cancel
          </Button>
        </Stack>
      </Stack>
    </FormPage>
  );
}
