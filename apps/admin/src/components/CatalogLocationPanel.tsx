import { http } from '@gaming-cafe/utils';
import { Alert, Button, MenuItem, Paper, Stack, TextField, Typography } from '@mui/material';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import CatalogLocationFields, { useCatalogLocations } from './CatalogLocationFields';

interface Scope {
  locationIds: string[];
  prices: { locationId: string; price: number }[];
  writableLocationIds?: string[];
}
export default function CatalogLocationPanel({
  kind,
  id,
  canWrite,
  onCanEditChange,
}: {
  kind: 'products' | 'plans';
  id: string;
  canWrite: boolean;
  onCanEditChange?: (allowed: boolean) => void;
}) {
  const context = useCatalogLocations();
  const client = useQueryClient();
  const [draft, setDraft] = useState<Scope>();
  const query = useQuery({
    queryKey: ['catalog-scope', kind, id],
    queryFn: async () => {
      const data = await http.get<Scope>(`/${kind}/${id}/location-scope`);
      return data;
    },
  });
  const data = draft ?? query.data;
  const writableLocations = context.locations.filter((l) =>
    query.data?.writableLocationIds?.includes(l.id),
  );
  const allowed =
    context.organizationAdmin ||
    (!!query.data?.locationIds.length &&
      query.data.locationIds.every((id) => writableLocations.some((l) => l.id === id)));
  const canEdit = canWrite && allowed;
  const canPrice = canWrite && writableLocations.length > 0;
  const save = useMutation({
    mutationFn: () =>
      http.put<Scope>(`/${kind}/${id}/location-scope`, {
        locationIds: data?.locationIds,
        prices: data?.prices.filter((p) => writableLocations.some((l) => l.id === p.locationId)),
      }),
    onSuccess: async () => {
      setDraft(undefined);
      await client.invalidateQueries();
    },
  });
  useEffect(() => {
    onCanEditChange?.(canEdit);
  }, [canEdit, onCanEditChange]);
  return (
    <Paper variant="outlined" sx={{ p: 3, my: 3 }}>
      <Typography variant="h6">Sharing and location prices</Typography>
      {query.error && <Alert severity="error">{query.error.message}</Alert>}
      {data && (
        <>
          {!allowed && (
            <Alert severity="info">
              An organization administrator manages this shared entry. You can use it at your
              assigned locations.
            </Alert>
          )}
          <CatalogLocationFields
            value={data.locationIds}
            disabled={!canEdit || save.isPending}
            onChange={(locationIds) =>
              setDraft({
                locationIds,
                prices: data.prices.filter(
                  (p) => !locationIds.length || locationIds.includes(p.locationId),
                ),
              })
            }
          />
          <Typography variant="body2" sx={{ mb: 2 }}>
            Locations use the catalog price unless an override is added below. Published pricing
            rules then apply to that price.
          </Typography>
          <Stack gap={2}>
            {data.prices.map((price, index) => (
              <Stack key={price.locationId} direction="row" gap={2}>
                <TextField
                  select
                  label="Location"
                  value={price.locationId}
                  disabled={
                    !canPrice || !query.data?.writableLocationIds?.includes(price.locationId)
                  }
                  sx={{ minWidth: 200 }}
                  onChange={(e) =>
                    setDraft({
                      ...data,
                      prices: data.prices.map((p, i) =>
                        i === index ? { ...p, locationId: e.target.value } : p,
                      ),
                    })
                  }
                >
                  {context.locations
                    .filter(
                      (l) =>
                        (l.id === price.locationId ||
                          writableLocations.some((w) => w.id === l.id)) &&
                        (!data.locationIds.length || data.locationIds.includes(l.id)) &&
                        !data.prices.some((p, i) => i !== index && p.locationId === l.id),
                    )
                    .map((l) => (
                      <MenuItem key={l.id} value={l.id}>
                        {l.name}
                      </MenuItem>
                    ))}
                </TextField>
                <TextField
                  label="Base price"
                  type="number"
                  value={price.price}
                  disabled={
                    !canPrice || !query.data?.writableLocationIds?.includes(price.locationId)
                  }
                  slotProps={{ htmlInput: { min: 0, step: 0.01 } }}
                  onChange={(e) =>
                    setDraft({
                      ...data,
                      prices: data.prices.map((p, i) =>
                        i === index ? { ...p, price: Number(e.target.value) } : p,
                      ),
                    })
                  }
                />
                {canPrice && query.data?.writableLocationIds?.includes(price.locationId) && (
                  <Button
                    onClick={() =>
                      setDraft({ ...data, prices: data.prices.filter((_, i) => i !== index) })
                    }
                  >
                    Use catalog price
                  </Button>
                )}
              </Stack>
            ))}
            {canPrice && (
              <Stack direction="row" gap={2}>
                <Button
                  onClick={() => {
                    const location = writableLocations.find(
                      (l) =>
                        (!data.locationIds.length || data.locationIds.includes(l.id)) &&
                        !data.prices.some((p) => p.locationId === l.id),
                    );
                    if (location)
                      setDraft({
                        ...data,
                        prices: [...data.prices, { locationId: location.id, price: 0 }],
                      });
                  }}
                >
                  Add location price
                </Button>
                <Button
                  variant="contained"
                  disabled={
                    !draft ||
                    save.isPending ||
                    data.prices.some((p) => !Number.isFinite(p.price) || p.price < 0)
                  }
                  onClick={() => save.mutate()}
                >
                  Save sharing and prices
                </Button>
              </Stack>
            )}
            {save.error && <Alert severity="error">{save.error.message}</Alert>}
            {save.isSuccess && !draft && (
              <Alert severity="success">Sharing and prices saved.</Alert>
            )}
          </Stack>
        </>
      )}
    </Paper>
  );
}
