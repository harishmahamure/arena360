import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  FormControlLabel,
  Stack,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';
import { createVendor } from '../../../services/vendors';

export default function VendorNewPage() {
  const navigate = useNavigate();
  const [error, setError] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);

  const [name, setName] = useState('');
  const [contactPerson, setContactPerson] = useState('');
  const [phone, setPhone] = useState('');
  const [email, setEmail] = useState('');
  const [address, setAddress] = useState('');
  const [gstNumber, setGstNumber] = useState('');
  const [isActive, setIsActive] = useState(true);

  const handleSubmit = async () => {
    setError(undefined);
    if (!name.trim()) {
      setError('Name is required');
      return;
    }

    setLoading(true);
    try {
      await createVendor({
        name: name.trim(),
        contactPerson: contactPerson || undefined,
        phone: phone || undefined,
        email: email || undefined,
        address: address || undefined,
        gstNumber: gstNumber || undefined,
        isActive,
      });
      navigate('/vendors');
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to create vendor');
    } finally {
      setLoading(false);
    }
  };

  return (
    <Box sx={{ p: 3, maxWidth: 600 }}>
      <Typography variant="h4" fontWeight={600} gutterBottom>
        New Vendor
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 3 }}>
        Add a new supplier or vendor
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
              label="Name"
              value={name}
              onChange={(e) => setName(e.target.value)}
              fullWidth
              required
            />
            <TextField
              label="Contact Person"
              value={contactPerson}
              onChange={(e) => setContactPerson(e.target.value)}
              fullWidth
            />
            <TextField
              label="Phone"
              value={phone}
              onChange={(e) => setPhone(e.target.value)}
              fullWidth
            />
            <TextField
              label="Email"
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              fullWidth
            />
            <TextField
              label="Address"
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              fullWidth
              multiline
              rows={2}
            />
            <TextField
              label="GST Number"
              value={gstNumber}
              onChange={(e) => setGstNumber(e.target.value)}
              fullWidth
            />
            <FormControlLabel
              control={
                <Switch checked={isActive} onChange={(e) => setIsActive(e.target.checked)} />
              }
              label="Active"
            />

            <Stack direction="row" spacing={2}>
              <Button variant="contained" onClick={handleSubmit} disabled={loading} fullWidth>
                {loading ? 'Creating...' : 'Create Vendor'}
              </Button>
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
        </CardContent>
      </Card>
    </Box>
  );
}
