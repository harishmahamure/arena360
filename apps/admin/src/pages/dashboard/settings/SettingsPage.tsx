import { toastUtils } from '@gaming-cafe/utils';
import {
  Box,
  Button,
  Card,
  CardContent,
  CardHeader,
  CircularProgress,
  Divider,
  Grid,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import { getConfigs, upsertConfig } from '../../../services/config';

interface ConfigGroup {
  title: string;
  category: string;
  fields: { key: string; label: string; type?: 'text' | 'number' | 'boolean' }[];
}

const CONFIG_GROUPS: ConfigGroup[] = [
  {
    title: 'Business Information',
    category: 'business',
    fields: [
      { key: 'business.name', label: 'Business Name' },
      { key: 'business.address', label: 'Address' },
      { key: 'business.phone', label: 'Phone' },
      { key: 'business.email', label: 'Email' },
      { key: 'business.gst_number', label: 'GST Number' },
      { key: 'business.logo_url', label: 'Logo URL' },
    ],
  },
  {
    title: 'Receipt Settings',
    category: 'receipt',
    fields: [
      { key: 'receipt.header_text', label: 'Header Text' },
      { key: 'receipt.footer_text', label: 'Footer Text' },
    ],
  },
  {
    title: 'Pricing Defaults',
    category: 'pricing',
    fields: [
      {
        key: 'pricing.default_per_minute_rate',
        label: 'Default Per-Minute Rate (INR)',
        type: 'number',
      },
      { key: 'pricing.currency', label: 'Currency Code' },
      { key: 'pricing.tax_rate', label: 'Tax Rate (%)', type: 'number' },
    ],
  },
];

export default function SettingsPage() {
  const queryClient = useQueryClient();
  const [values, setValues] = useState<Record<string, string>>({});
  const [saving, setSaving] = useState<string | null>(null);

  const { data: configs, isLoading } = useQuery({
    queryKey: ['configs'],
    queryFn: () => getConfigs(),
  });

  useEffect(() => {
    if (configs) {
      const v: Record<string, string> = {};
      for (const config of configs) {
        const raw = config.value;
        v[config.key] = typeof raw === 'string' ? raw : JSON.stringify(raw);
      }
      setValues(v);
    }
  }, [configs]);

  const handleSave = async (key: string) => {
    const rawValue = values[key] ?? '';
    let parsedValue: unknown = rawValue;

    const field = CONFIG_GROUPS.flatMap((g) => g.fields).find((f) => f.key === key);
    if (field?.type === 'number') {
      parsedValue = Number.parseFloat(rawValue) || 0;
    }

    setSaving(key);
    try {
      await upsertConfig(key, parsedValue);
      toastUtils.success(`"${key}" saved`);
      queryClient.invalidateQueries({ queryKey: ['configs'] });
    } catch {
      toastUtils.error(`Failed to save "${key}"`);
    } finally {
      setSaving(null);
    }
  };

  if (isLoading) {
    return (
      <Box sx={{ display: 'flex', justifyContent: 'center', py: 10 }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Box sx={{ py: { xs: 3, md: 4 }, px: { xs: 2, sm: 3, md: 4 } }}>
      <Typography variant="h3" fontWeight={700} gutterBottom>
        Settings
      </Typography>
      <Typography variant="body1" color="text.secondary" sx={{ mb: 4 }}>
        Manage your business configuration
      </Typography>

      <Grid container spacing={3}>
        {CONFIG_GROUPS.map((group) => (
          <Grid key={group.category} size={{ xs: 12, md: 6 }}>
            <Card variant="outlined">
              <CardHeader title={group.title} />
              <Divider />
              <CardContent sx={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
                {group.fields.map((field) => (
                  <Box key={field.key} sx={{ display: 'flex', gap: 1, alignItems: 'flex-end' }}>
                    <TextField
                      label={field.label}
                      size="small"
                      fullWidth
                      type={field.type === 'number' ? 'number' : 'text'}
                      value={values[field.key] ?? ''}
                      onChange={(e) =>
                        setValues((prev) => ({ ...prev, [field.key]: e.target.value }))
                      }
                    />
                    <Button
                      variant="outlined"
                      size="small"
                      disabled={saving === field.key}
                      onClick={() => handleSave(field.key)}
                      sx={{ minWidth: 70, height: 40 }}
                    >
                      {saving === field.key ? <CircularProgress size={18} /> : 'Save'}
                    </Button>
                  </Box>
                ))}
              </CardContent>
            </Card>
          </Grid>
        ))}
      </Grid>
    </Box>
  );
}
