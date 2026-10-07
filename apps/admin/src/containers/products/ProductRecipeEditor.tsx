import { toastUtils } from '@gaming-cafe/utils';
import { Add, DeleteOutline } from '@mui/icons-material';
import {
  Alert,
  Box,
  Button,
  FormControlLabel,
  IconButton,
  MenuItem,
  Paper,
  Stack,
  Switch,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useProductUnits } from '../../hooks/useProductUnits';
import { getProducts } from '../../services/product/list';
import {
  getProductRecipe,
  type ProductOption,
  type ProductOptionGroup,
  type ProductRecipe,
  type RecipeIngredient,
  saveProductRecipe,
} from '../../services/product/recipe';
import { metricUnitConversion } from './productUnits';

const EMPTY_RECIPE: ProductRecipe = { items: [], optionGroups: [] };

type IngredientChoice = { id: string; label: string; unit: string; conversion?: string };

function IngredientRows({
  rows,
  choices,
  disabled,
  allowNegative,
  onChange,
}: {
  rows: RecipeIngredient[];
  choices: IngredientChoice[];
  disabled: boolean;
  allowNegative: boolean;
  onChange: (rows: RecipeIngredient[]) => void;
}) {
  const edit = (index: number, patch: Partial<RecipeIngredient>) =>
    onChange(rows.map((row, i) => (i === index ? { ...row, ...patch } : row)));
  return (
    <Stack spacing={1}>
      {rows.map((row, index) => {
        const choice = choices.find((item) => item.id === row.ingredientId);
        const unit = choice?.unit ?? 'units';
        return (
          // biome-ignore lint/suspicious/noArrayIndexKey: rows have no ID and may repeat an ingredient while being edited
          <Stack key={`${row.ingredientId}-${index}`} direction="row" spacing={1}>
            <TextField
              select
              size="small"
              label="Ingredient"
              value={row.ingredientId}
              disabled={disabled}
              onChange={(event) => edit(index, { ingredientId: event.target.value })}
              sx={{ flex: 2 }}
            >
              {choices.map((choice) => (
                <MenuItem key={choice.id} value={choice.id}>
                  {choice.label}
                </MenuItem>
              ))}
            </TextField>
            <TextField
              size="small"
              type="number"
              label={allowNegative ? `Change (${unit}; − removes)` : `Quantity (${unit})`}
              value={row.quantity}
              disabled={disabled}
              onChange={(event) =>
                edit(index, { quantity: Math.trunc(Number(event.target.value)) })
              }
              slotProps={{ htmlInput: { step: 1, min: allowNegative ? undefined : 1 } }}
              helperText={choice?.conversion ?? `Per item sold, in ${unit}`}
              sx={{ flex: 1 }}
            />
            <IconButton
              aria-label="Remove ingredient"
              disabled={disabled}
              onClick={() => onChange(rows.filter((_, i) => i !== index))}
            >
              <DeleteOutline fontSize="small" />
            </IconButton>
          </Stack>
        );
      })}
      <Button
        size="small"
        startIcon={<Add />}
        disabled={disabled || choices.length === 0}
        onClick={() => onChange([...rows, { ingredientId: choices[0]?.id ?? '', quantity: 1 }])}
        sx={{ alignSelf: 'flex-start' }}
      >
        Add ingredient
      </Button>
    </Stack>
  );
}

/** Ingredients deducted per item sold, and the options a cashier picks at the POS. */
export default function ProductRecipeEditor({
  productId,
  isRawMaterial,
  canWrite,
}: {
  productId: string;
  isRawMaterial: boolean;
  canWrite: boolean;
}) {
  const queryClient = useQueryClient();
  const { units } = useProductUnits();
  const [draft, setDraft] = useState<ProductRecipe>(EMPTY_RECIPE);
  const [error, setError] = useState<string>();
  const [saving, setSaving] = useState(false);

  const recipeQuery = useQuery({
    queryKey: ['product-recipe', productId],
    queryFn: () => getProductRecipe(productId),
  });
  const productsQuery = useQuery({
    queryKey: ['recipe-ingredient-products'],
    queryFn: () => getProducts({ limit: 100, sortBy: 'name', sortOrder: 'ASC' }),
  });

  useEffect(() => {
    if (recipeQuery.data) setDraft(recipeQuery.data);
  }, [recipeQuery.data]);

  const choices = useMemo(() => {
    const unitById = new Map(units.map((unit) => [unit.id, unit]));
    return (productsQuery.data?.data ?? [])
      .filter((product) => product.id !== productId)
      .sort((a, b) => Number(b.isRawMaterial ?? false) - Number(a.isRawMaterial ?? false))
      .map((product) => {
        const unit = product.unitId ? unitById.get(product.unitId) : undefined;
        const purchase = product.purchaseUnitId ? unitById.get(product.purchaseUnitId) : undefined;
        const conversion = metricUnitConversion(purchase?.type, unit?.type);
        return {
          id: product.id,
          label: `${product.name}${unit ? ` (${unit.abbreviation})` : ''}${product.isRawMaterial ? '' : ' · sold item'}`,
          unit: unit?.abbreviation ?? 'units',
          conversion:
            conversion !== undefined && conversion > 1
              ? `1 ${purchase?.abbreviation} = ${conversion.toLocaleString()} ${unit?.abbreviation}`
              : undefined,
        };
      });
  }, [productsQuery.data, productId, units]);

  const disabled = !canWrite || saving;
  const editGroup = (index: number, patch: Partial<ProductOptionGroup>) =>
    setDraft({
      ...draft,
      optionGroups: draft.optionGroups.map((group, i) =>
        i === index ? { ...group, ...patch } : group,
      ),
    });
  const editOption = (groupIndex: number, optionIndex: number, patch: Partial<ProductOption>) => {
    const group = draft.optionGroups[groupIndex];
    if (!group) return;
    editGroup(groupIndex, {
      options: group.options.map((option, i) =>
        i === optionIndex ? { ...option, ...patch } : option,
      ),
    });
  };

  const save = async () => {
    setSaving(true);
    setError(undefined);
    try {
      const saved = await saveProductRecipe(productId, draft);
      queryClient.setQueryData(['product-recipe', productId], saved);
      toastUtils.success('Recipe saved');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to save recipe');
    } finally {
      setSaving(false);
    }
  };

  if (isRawMaterial) {
    return (
      <Alert severity="info" sx={{ mt: 3 }}>
        This is a raw material. Add it as an ingredient in another product's recipe; raw materials
        cannot have their own recipe or options.
      </Alert>
    );
  }

  return (
    <Paper variant="outlined" sx={{ p: 3, mt: 3 }}>
      <Typography variant="h6" fontWeight={600}>
        Recipe & options
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        With ingredients, each sale deducts them from store stock instead of this product's own
        stock. Enter each portion in the displayed stock unit, such as 20 g of paneer. Purchasing 1
        kg adds 1,000 g when the stock unit is grams.
      </Typography>
      {recipeQuery.isError && <Alert severity="error">Could not load the recipe.</Alert>}
      {error && (
        <Alert severity="error" sx={{ mb: 2 }} onClose={() => setError(undefined)}>
          {error}
        </Alert>
      )}

      <Typography fontWeight={600} sx={{ mb: 1 }}>
        Ingredients per item
      </Typography>
      <IngredientRows
        rows={draft.items}
        choices={choices}
        disabled={disabled}
        allowNegative={false}
        onChange={(items) => setDraft({ ...draft, items })}
      />

      <Typography fontWeight={600} sx={{ mt: 3, mb: 1 }}>
        Option groups
      </Typography>
      <Stack spacing={2}>
        {draft.optionGroups.map((group, groupIndex) => (
          <Box
            key={group.id ?? `new-${groupIndex}`}
            sx={{ p: 2, border: 1, borderColor: 'divider', borderRadius: 2 }}
          >
            <Stack direction={{ xs: 'column', sm: 'row' }} spacing={1} alignItems="center">
              <TextField
                size="small"
                label="Group name"
                placeholder="Patty, Extras…"
                value={group.name}
                disabled={disabled}
                onChange={(event) => editGroup(groupIndex, { name: event.target.value })}
                sx={{ flex: 1 }}
              />
              <FormControlLabel
                label="Required"
                control={
                  <Switch
                    checked={group.required}
                    disabled={disabled}
                    onChange={(_, required) => editGroup(groupIndex, { required })}
                  />
                }
              />
              <FormControlLabel
                label="Pick several"
                control={
                  <Switch
                    checked={group.multiple}
                    disabled={disabled}
                    onChange={(_, multiple) => editGroup(groupIndex, { multiple })}
                  />
                }
              />
              <Tooltip title="Remove group">
                <span>
                  <IconButton
                    aria-label={`Remove ${group.name || 'group'}`}
                    disabled={disabled}
                    onClick={() =>
                      setDraft({
                        ...draft,
                        optionGroups: draft.optionGroups.filter((_, i) => i !== groupIndex),
                      })
                    }
                  >
                    <DeleteOutline fontSize="small" />
                  </IconButton>
                </span>
              </Tooltip>
            </Stack>
            <Stack spacing={2} sx={{ mt: 2, pl: { sm: 2 } }}>
              {group.options.map((option, optionIndex) => (
                <Box key={option.id ?? `new-${optionIndex}`}>
                  <Stack direction="row" spacing={1}>
                    <TextField
                      size="small"
                      label="Option"
                      placeholder="Double patty"
                      value={option.name}
                      disabled={disabled}
                      onChange={(event) =>
                        editOption(groupIndex, optionIndex, { name: event.target.value })
                      }
                      sx={{ flex: 2 }}
                    />
                    <TextField
                      size="small"
                      type="number"
                      label="Price change (₹)"
                      value={option.priceDelta}
                      disabled={disabled}
                      onChange={(event) =>
                        editOption(groupIndex, optionIndex, {
                          priceDelta: Number(event.target.value),
                        })
                      }
                      sx={{ flex: 1 }}
                    />
                    <IconButton
                      aria-label={`Remove ${option.name || 'option'}`}
                      disabled={disabled}
                      onClick={() =>
                        editGroup(groupIndex, {
                          options: group.options.filter((_, i) => i !== optionIndex),
                        })
                      }
                    >
                      <DeleteOutline fontSize="small" />
                    </IconButton>
                  </Stack>
                  <Box sx={{ pl: 2, mt: 1 }}>
                    <IngredientRows
                      rows={option.ingredients}
                      choices={choices}
                      disabled={disabled}
                      allowNegative
                      onChange={(ingredients) =>
                        editOption(groupIndex, optionIndex, { ingredients })
                      }
                    />
                  </Box>
                </Box>
              ))}
              <Button
                size="small"
                variant="outlined"
                startIcon={<Add />}
                disabled={disabled}
                onClick={() =>
                  editGroup(groupIndex, {
                    options: [...group.options, { name: '', priceDelta: 0, ingredients: [] }],
                  })
                }
                sx={{ alignSelf: 'flex-start' }}
              >
                Add option
              </Button>
            </Stack>
          </Box>
        ))}
        <Button
          variant="outlined"
          startIcon={<Add />}
          disabled={disabled}
          onClick={() =>
            setDraft({
              ...draft,
              optionGroups: [
                ...draft.optionGroups,
                { name: '', required: false, multiple: false, options: [] },
              ],
            })
          }
          sx={{ alignSelf: 'flex-start' }}
        >
          Add option group
        </Button>
      </Stack>

      {canWrite && (
        <Stack direction="row" justifyContent="flex-end" spacing={1} sx={{ mt: 3 }}>
          <Button disabled={saving} onClick={() => setDraft(recipeQuery.data ?? EMPTY_RECIPE)}>
            Discard changes
          </Button>
          <Button variant="contained" disabled={saving || recipeQuery.isLoading} onClick={save}>
            Save recipe
          </Button>
        </Stack>
      )}
    </Paper>
  );
}
