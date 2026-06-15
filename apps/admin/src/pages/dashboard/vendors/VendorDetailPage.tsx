import { PhoneField } from '@gaming-cafe/ui';
import { digitsOnly, toastUtils, trimValue } from '@gaming-cafe/utils';
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControlLabel,
  Stack,
  Switch,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import { useNavigate, useParams } from 'react-router-dom';
import { deleteVendor, getVendor, updateVendor, type Vendor } from '../../../services/vendors';

export default function VendorDetailPage() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const queryClient = useQueryClient();

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | undefined>();
  const [deleteDialogOpen, setDeleteDialogOpen] = useState(false);

  const [name, setName] = useState('');
  const [contactPerson, setContactPerson] = useState('');
  const [phone, setPhone] = useState('');
  const [email, setEmail] = useState('');
  const [address, setAddress] = useState('');
  const [gstNumber, setGstNumber] = useState('');
  const [isActive, setIsActive] = useState(true);
  const [formLoaded, setFormLoaded] = useState(false);

  const { data: vendor, isLoading } = useQuery({
    queryKey: ['vendor', id],
    queryFn: () => getVendor(id as string),
    enabled: !!id,
  });

  const populateForm = (v: Vendor) => {
    if (!formLoaded) {
      setName(v.name);
      setContactPerson(v.contactPerson || '');
      setPhone(v.phone || '');
      setEmail(v.email || '');
      setAddress(v.address || '');
      setGstNumber(v.gstNumber || '');
      setIsActive(v.isActive);
      setFormLoaded(true);
    }
  };

  if (vendor && !formLoaded) {
    populateForm(vendor);
  }

  const handleSave = async () => {
    if (!id) return;
    if (!name.trim()) {
      setError('Name is required');
      return;
    }
    setSaving(true);
    setError(undefined);
    try {
      await updateVendor(id, {
        name: trimValue(name),
        contactPerson: trimValue(contactPerson) || undefined,
        phone: digitsOnly(phone) || undefined,
        email: trimValue(email) || undefined,
        address: trimValue(address) || undefined,
        gstNumber: trimValue(gstNumber) || undefined,
        isActive,
      });
      toastUtils.success('Vendor updated');
      queryClient.invalidateQueries({ queryKey: ['vendor', id] });
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to update vendor');
    } finally {
      setSaving(false);
    }
  };

  const handleDeleteConfirm = async () => {
    if (!id) return;
    setSaving(true);
    try {
      await deleteVendor(id);
      toastUtils.success('Vendor deleted');
      navigate('/vendors');
    } catch {
      toastUtils.error('Failed to delete vendor');
    } finally {
      setSaving(false);
      setDeleteDialogOpen(false);
    }
  };

  if (isLoading) {
    return (
      <Box sx={{ p: 3 }}>
        <Typography>Loading...</Typography>
      </Box>
    );
  }

  if (!vendor) {
    return (
      <Box sx={{ p: 3 }}>
        <Alert severity="error">Vendor not found</Alert>
      </Box>
    );
  }

  return (
    <Box sx={{ p: 3, maxWidth: 600 }}>
      <Typography variant="h4" fontWeight={600} gutterBottom>
        Edit Vendor
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
            <PhoneField
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
              <Button variant="contained" onClick={handleSave} disabled={saving}>
                {saving ? 'Saving...' : 'Save'}
              </Button>
              <Button
                variant="contained"
                color="error"
                onClick={() => setDeleteDialogOpen(true)}
                disabled={saving}
              >
                Delete
              </Button>
              <Button variant="outlined" onClick={() => navigate('/vendors')}>
                Back
              </Button>
            </Stack>
          </Stack>
        </CardContent>
      </Card>

      <Dialog open={deleteDialogOpen} onClose={() => setDeleteDialogOpen(false)}>
        <DialogTitle>Delete Vendor</DialogTitle>
        <DialogContent>
          <Typography>
            Are you sure you want to delete &quot;{vendor.name}&quot;? This action cannot be undone.
          </Typography>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setDeleteDialogOpen(false)}>Cancel</Button>
          <Button color="error" variant="contained" onClick={handleDeleteConfirm} disabled={saving}>
            Delete
          </Button>
        </DialogActions>
      </Dialog>
    </Box>
  );
}
