import { PageShell } from '@gaming-cafe/ui';
import {
  AccountBalance,
  Add as AddIcon,
  ArrowBack,
  CallSplit,
  ShoppingCart as CartIcon,
  CreditCard,
  Delete as DeleteIcon,
  Nightlight,
  Payments,
  Remove as RemoveIcon,
  Search as SearchIcon,
  Store as StoreIcon,
} from '@mui/icons-material';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Card,
  CardActionArea,
  CardContent,
  Chip,
  CircularProgress,
  Divider,
  Grid,
  IconButton,
  InputAdornment,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { Link as RouterLink, useNavigate } from 'react-router-dom';
import { ActiveShiftGuard } from '../../../components/ActiveShiftGuard';
import CreditEligibilityAlert from '../../../components/CreditEligibilityAlert';
import {
  type PaymentMethodType,
  PaymentMethodValues,
  paymentMethodOptions,
} from '../../../containers/transactions/schemas/transaction-schema';
import { getPlayerCredit } from '../../../services/credit';
import { getInventoryLocations, getLocationStock } from '../../../services/inventory';
import { getPlayers } from '../../../services/players/list';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import { addTransaction } from '../../../services/transaction/add';
import { PaymentStatus, TransactionType } from '../../../services/transaction/list';
import { effectiveProductPrice, isNightPricingWindow } from '../../../utils/pricing';

interface Product {
  id: string;
  name: string;
  description: string;
  price: number;
  dayPrice: number;
  nightPrice: number;
  stockQuantity: number;
  unitsPerPurchaseUnit: number;
}

interface CartItem extends Product {
  quantity: number;
  effectivePrice: number;
}

interface Player {
  id: string;
  username: string;
}

const PAYMENT_TILE_ICONS: Record<string, typeof Payments> = {
  [PaymentMethodValues.CASH]: Payments,
  [PaymentMethodValues.ONLINE]: CreditCard,
  [PaymentMethodValues.SPLIT_PAYMENT]: CallSplit,
  [PaymentMethodValues.CREDIT]: AccountBalance,
};

function mapProductForPos(product: ProductResponse, stockByProduct: Map<string, number>): Product {
  const dayPrice = product.dayPrice ?? parseFloat(product.price);
  const nightPrice = product.nightPrice ?? dayPrice;
  const effective = effectiveProductPrice(dayPrice, nightPrice);
  const stock = stockByProduct.get(product.id) ?? product.stockQuantity ?? 0;
  return {
    id: product.id,
    name: product.name,
    description: product.description,
    price: effective,
    dayPrice,
    nightPrice,
    stockQuantity: stock,
    unitsPerPurchaseUnit: product.unitsPerPurchaseUnit ?? 1,
  };
}

function PosToolbar({
  saleLocationId,
  storeLocations,
  nightActive,
  onLocationChange,
}: {
  saleLocationId: string;
  storeLocations: { id: string; name: string }[];
  nightActive: boolean;
  onLocationChange: (id: string) => void;
}) {
  return (
    <Box sx={{ display: 'flex', gap: 2, flexWrap: 'wrap', alignItems: 'center' }}>
      <StoreIcon color="action" />
      <TextField
        select
        label="Store"
        size="small"
        value={saleLocationId}
        onChange={(e) => onLocationChange(e.target.value)}
        sx={{ minWidth: 220 }}
      >
        {storeLocations.map((loc) => (
          <MenuItem key={loc.id} value={loc.id}>
            {loc.name}
          </MenuItem>
        ))}
      </TextField>
      {nightActive && (
        <Chip
          icon={<Nightlight />}
          label="Night price active (11 PM – 8 AM)"
          color="secondary"
          size="small"
        />
      )}
    </Box>
  );
}

function PosProductCard({
  product,
  cartQuantity,
  nightActive,
  onAdd,
}: {
  product: Product;
  cartQuantity: number;
  nightActive: boolean;
  onAdd: (product: Product) => void;
}) {
  const inCart = cartQuantity > 0;
  const outOfStock = product.stockQuantity <= 0;

  return (
    <Card
      variant="outlined"
      sx={{
        height: '100%',
        opacity: outOfStock ? 0.5 : 1,
        pointerEvents: outOfStock ? 'none' : 'auto',
        borderColor: inCart ? 'primary.main' : 'divider',
        borderWidth: inCart ? 2 : 1,
        bgcolor: inCart ? 'action.selected' : 'background.paper',
      }}
    >
      <CardActionArea
        onClick={() => onAdd(product)}
        sx={{ minHeight: 44, height: '100%', alignItems: 'stretch' }}
        disabled={outOfStock}
      >
        <CardContent sx={{ position: 'relative', p: 2 }}>
          {inCart && (
            <Chip
              label={cartQuantity}
              size="small"
              color="primary"
              sx={{ position: 'absolute', top: 8, right: 8 }}
            />
          )}
          <Typography variant="body2" fontWeight={600} sx={{ pr: inCart ? 4 : 0 }} noWrap>
            {product.name}
          </Typography>
          {product.description && (
            <Typography variant="caption" color="text.secondary" display="block" noWrap>
              {product.description}
            </Typography>
          )}
          <Typography variant="body2" fontWeight={600} sx={{ mt: 1 }}>
            ₹{product.price.toFixed(2)}
            {nightActive && product.nightPrice !== product.dayPrice && (
              <Typography
                component="span"
                variant="caption"
                color="secondary.main"
                sx={{ ml: 0.5 }}
              >
                night
              </Typography>
            )}
          </Typography>
          <Typography
            variant="caption"
            color={product.stockQuantity > 0 ? 'success.main' : 'error.main'}
            display="block"
          >
            {product.stockQuantity} pcs in store
          </Typography>
          {product.unitsPerPurchaseUnit > 1 && (
            <Typography variant="caption" display="block" color="text.secondary">
              1 box = {product.unitsPerPurchaseUnit} pcs
            </Typography>
          )}
        </CardContent>
      </CardActionArea>
    </Card>
  );
}

function PosPaymentTiles({
  value,
  onChange,
}: {
  value: string;
  onChange: (method: string) => void;
}) {
  return (
    <Grid container spacing={1.5} sx={{ mb: 2 }}>
      {paymentMethodOptions.map((option) => {
        const Icon = PAYMENT_TILE_ICONS[option.value] ?? Payments;
        const selected = value === option.value;
        return (
          <Grid key={option.value} size={{ xs: 12, sm: 6 }}>
            <Card
              variant="outlined"
              sx={{
                borderWidth: selected ? 2 : 1,
                borderColor: selected ? 'primary.main' : 'divider',
                bgcolor: selected ? 'action.selected' : 'background.paper',
                cursor: 'pointer',
              }}
              onClick={() => onChange(option.value)}
            >
              <CardActionArea sx={{ minHeight: 56 }}>
                <CardContent
                  sx={{
                    display: 'flex',
                    alignItems: 'center',
                    gap: 1.5,
                    py: 1.5,
                    '&:last-child': { pb: 1.5 },
                  }}
                >
                  <Icon color={selected ? 'primary' : 'action'} />
                  <Typography variant="body2" fontWeight={selected ? 600 : 500}>
                    {option.label}
                  </Typography>
                </CardContent>
              </CardActionArea>
            </Card>
          </Grid>
        );
      })}
    </Grid>
  );
}

export default function CreateProductTransactionPage() {
  const navigate = useNavigate();
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);

  const [selectedPlayer, setSelectedPlayer] = useState<Player | null>(null);
  const [playerOptions, setPlayerOptions] = useState<Player[]>([]);
  const [playerInputValue, setPlayerInputValue] = useState('');
  const [playerLoading, setPlayerLoading] = useState(false);

  const [productSearch, setProductSearch] = useState('');
  const [cart, setCart] = useState<CartItem[]>([]);
  const [paymentMethod, setPaymentMethod] = useState<string>(PaymentMethodValues.CASH);
  const [cashAmount, setCashAmount] = useState<string>('');
  const [onlineAmount, setOnlineAmount] = useState<string>('');
  const [notes, setNotes] = useState<string>('');
  const [saleLocationId, setSaleLocationId] = useState<string>('');

  const nightActive = isNightPricingWindow();

  const { data: locationsData } = useQuery({
    queryKey: ['pos-store-locations'],
    queryFn: () => getInventoryLocations({ kind: 'store', isActive: true, limit: 20 }),
  });

  const storeLocations = locationsData?.data ?? [];

  useEffect(() => {
    if (!saleLocationId && storeLocations.length > 0) {
      setSaleLocationId(storeLocations[0]?.id ?? '');
    }
  }, [storeLocations, saleLocationId]);

  const { data: storeStockData } = useQuery({
    queryKey: ['pos-store-stock', saleLocationId],
    queryFn: () => getLocationStock({ locationId: saleLocationId, limit: 500 }),
    enabled: !!saleLocationId,
  });

  const stockByProduct = useMemo(() => {
    const map = new Map<string, number>();
    for (const row of storeStockData?.data ?? []) {
      map.set(row.productId, row.quantityPieces);
    }
    return map;
  }, [storeStockData]);

  const { data: productsData, isLoading: productsLoading } = useQuery({
    queryKey: ['pos-products', saleLocationId],
    queryFn: () => getProducts({ limit: 200, sortBy: 'name', sortOrder: 'ASC' }),
    enabled: !!saleLocationId,
  });

  const catalogProducts = useMemo(() => {
    return (productsData?.data ?? [])
      .filter((p) => p.isActive)
      .map((product) => mapProductForPos(product, stockByProduct));
  }, [productsData, stockByProduct]);

  const filteredProducts = useMemo(() => {
    const query = productSearch.trim().toLowerCase();
    if (!query) return catalogProducts;
    return catalogProducts.filter(
      (p) => p.name.toLowerCase().includes(query) || p.description.toLowerCase().includes(query),
    );
  }, [catalogProducts, productSearch]);

  const cartQtyByProduct = useMemo(() => {
    const map = new Map<string, number>();
    for (const item of cart) {
      map.set(item.id, item.quantity);
    }
    return map;
  }, [cart]);

  useEffect(() => {
    if (playerInputValue.length < 2) {
      setPlayerOptions([]);
      return;
    }

    const searchPlayers = async () => {
      setPlayerLoading(true);
      try {
        const data = await getPlayers({
          limit: 100,
          username: playerInputValue,
          isActive: 1,
          sortBy: 'username',
          sortOrder: 'ASC',
        });
        setPlayerOptions(
          data.data.map((player) => ({
            id: player.id,
            username: player.username,
          })),
        );
      } catch (_err) {
      } finally {
        setPlayerLoading(false);
      }
    };

    const timeoutId = setTimeout(searchPlayers, 300);
    return () => clearTimeout(timeoutId);
  }, [playerInputValue]);

  const addToCart = (product: Product) => {
    const effectivePrice = effectiveProductPrice(product.dayPrice, product.nightPrice);
    const cartProduct = { ...product, price: effectivePrice, effectivePrice };
    const existingItem = cart.find((item) => item.id === product.id);
    if (existingItem) {
      updateQuantity(product.id, existingItem.quantity + 1);
    } else {
      setCart([...cart, { ...cartProduct, quantity: 1 }]);
    }
  };

  const updateQuantity = (productId: string, newQuantity: number) => {
    const item = cart.find((item) => item.id === productId);
    if (!item) return;

    if (newQuantity <= 0) {
      removeFromCart(productId);
      return;
    }

    if (newQuantity > item.stockQuantity) {
      setError(`Only ${item.stockQuantity} units available in stock`);
      return;
    }

    setCart(
      cart.map((item) => (item.id === productId ? { ...item, quantity: newQuantity } : item)),
    );
  };

  const removeFromCart = (productId: string) => {
    setCart(cart.filter((item) => item.id !== productId));
  };

  const clearCart = () => {
    setCart([]);
  };

  const subtotal = cart.reduce((sum, item) => sum + item.effectivePrice * item.quantity, 0);
  const total = subtotal;

  const isCredit = paymentMethod === PaymentMethodValues.CREDIT;
  const { data: creditDetail } = useQuery({
    queryKey: ['player-credit', selectedPlayer?.id],
    queryFn: () => {
      if (!selectedPlayer) throw new Error('No player selected');
      return getPlayerCredit(selectedPlayer.id);
    },
    enabled: isCredit && !!selectedPlayer,
  });

  const creditBlocked = useMemo(() => {
    if (!isCredit || !creditDetail?.summary) return isCredit;
    const s = creditDetail.summary;
    if (!s.creditEnabled || s.creditLimit <= 0) return true;
    return total > s.available + 0.001;
  }, [isCredit, creditDetail, total]);

  const handleSubmit = async () => {
    setError(undefined);
    setSuccess(undefined);

    if (!selectedPlayer) {
      setError('Please select a player');
      return;
    }

    if (cart.length === 0) {
      setError('Please add at least one product to the cart');
      return;
    }

    if (paymentMethod === PaymentMethodValues.SPLIT_PAYMENT && (!cashAmount || !onlineAmount)) {
      setError('Please enter both cash and online amounts for split payment');
      return;
    }

    if (isCredit && creditBlocked) {
      setError('This player is not eligible for this credit purchase');
      return;
    }

    if (!saleLocationId) {
      setError('Please select a store location');
      return;
    }

    setLoading(true);

    try {
      const response = await addTransaction({
        playerId: selectedPlayer.id,
        saleLocationId,
        transactionType: TransactionType.PRODUCT_PURCHASE,
        amount: total,
        paymentStatus:
          paymentMethod === PaymentMethodValues.CREDIT
            ? PaymentStatus.CREDIT
            : PaymentStatus.COMPLETED,
        paymentMethod: paymentMethod as PaymentMethodType,
        cashAmount:
          paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
            ? parseFloat(cashAmount)
            : paymentMethod === PaymentMethodValues.CASH
              ? total
              : undefined,
        onlineAmount:
          paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
            ? parseFloat(onlineAmount)
            : paymentMethod === PaymentMethodValues.ONLINE
              ? total
              : undefined,
        notes: notes || undefined,
        lineItems: cart.map((item) => ({
          productId: item.id,
          quantity: item.quantity,
          unitPrice: item.effectivePrice,
        })),
      });

      if (response.paymentStatus === PaymentStatus.COMPLETED) {
        setSuccess('Transaction created successfully!');
        setTimeout(() => {
          navigate('/product-transactions');
        }, 1500);
      } else {
        setError('Transaction was created but payment is not completed.');
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : 'Failed to create transaction');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/product-transactions');
  };

  const handleLocationChange = (id: string) => {
    setSaleLocationId(id);
    setCart([]);
  };

  return (
    <ActiveShiftGuard>
      <PageShell
        header={
          <Box sx={{ mb: 2 }}>
            <Button
              component={RouterLink}
              to="/product-transactions"
              startIcon={<ArrowBack />}
              sx={{ ml: -1 }}
            >
              POS sales
            </Button>
          </Box>
        }
        toolbar={
          <PosToolbar
            saleLocationId={saleLocationId}
            storeLocations={storeLocations}
            nightActive={nightActive}
            onLocationChange={handleLocationChange}
          />
        }
      >
        {error && (
          <Alert severity="error" sx={{ mb: 3 }} onClose={() => setError(undefined)}>
            {error}
          </Alert>
        )}

        {success && (
          <Alert severity="success" sx={{ mb: 3 }}>
            {success}
          </Alert>
        )}

        <Grid container spacing={3}>
          <Grid size={{ xs: 12, md: 7 }}>
            <Card variant="outlined" sx={{ mb: 3 }}>
              <CardContent>
                <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
                  Select player
                </Typography>
                <Autocomplete
                  options={playerOptions}
                  getOptionLabel={(option) => option.username}
                  value={selectedPlayer}
                  onChange={(_, newValue) => setSelectedPlayer(newValue)}
                  inputValue={playerInputValue}
                  onInputChange={(_, newValue) => setPlayerInputValue(newValue)}
                  loading={playerLoading}
                  renderInput={(params) => (
                    <TextField {...params} placeholder="Search player by username..." fullWidth />
                  )}
                  renderOption={(props, option) => (
                    <li {...props} key={option.id}>
                      <Typography variant="body1">{option.username}</Typography>
                    </li>
                  )}
                />
              </CardContent>
            </Card>

            <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
              Products
            </Typography>
            <TextField
              placeholder="Search products..."
              value={productSearch}
              onChange={(e) => setProductSearch(e.target.value)}
              fullWidth
              size="small"
              sx={{ mb: 2 }}
              InputProps={{
                startAdornment: (
                  <InputAdornment position="start">
                    <SearchIcon fontSize="small" color="action" />
                  </InputAdornment>
                ),
              }}
            />

            {productsLoading ? (
              <Box sx={{ display: 'flex', justifyContent: 'center', py: 6 }}>
                <CircularProgress size={32} />
              </Box>
            ) : filteredProducts.length === 0 ? (
              <Typography
                variant="body2"
                color="text.secondary"
                sx={{ py: 4, textAlign: 'center' }}
              >
                {productSearch ? 'No products match your search' : 'No products available'}
              </Typography>
            ) : (
              <Grid container spacing={2}>
                {filteredProducts.map((product) => (
                  <Grid key={product.id} size={{ xs: 6, sm: 4, md: 3 }}>
                    <PosProductCard
                      product={product}
                      cartQuantity={cartQtyByProduct.get(product.id) ?? 0}
                      nightActive={nightActive}
                      onAdd={addToCart}
                    />
                  </Grid>
                ))}
              </Grid>
            )}
          </Grid>

          <Grid size={{ xs: 12, md: 5 }}>
            <Box
              sx={{
                position: { md: 'sticky' },
                top: { md: 80 },
                alignSelf: 'flex-start',
              }}
            >
              <Card variant="outlined" sx={{ mb: 3 }}>
                <CardContent>
                  <Box
                    sx={{
                      display: 'flex',
                      justifyContent: 'space-between',
                      alignItems: 'center',
                      mb: 2,
                    }}
                  >
                    <Typography variant="subtitle1" fontWeight={600}>
                      Cart ({cart.length} items)
                    </Typography>
                    {cart.length > 0 && (
                      <Button
                        size="small"
                        color="error"
                        onClick={clearCart}
                        startIcon={<DeleteIcon />}
                      >
                        Clear
                      </Button>
                    )}
                  </Box>

                  {cart.length === 0 ? (
                    <Box sx={{ textAlign: 'center', py: 4, color: 'text.secondary' }}>
                      <CartIcon sx={{ fontSize: 48, opacity: 0.3, mb: 1 }} />
                      <Typography variant="body2">Cart is empty</Typography>
                    </Box>
                  ) : (
                    <Box sx={{ maxHeight: { md: '40vh', lg: 360 }, overflowY: 'auto' }}>
                      {cart.map((item) => (
                        <Box
                          key={item.id}
                          sx={{
                            mb: 2,
                            pb: 2,
                            borderBottom: '1px solid',
                            borderColor: 'divider',
                            '&:last-child': { border: 'none' },
                          }}
                        >
                          <Box
                            sx={{
                              display: 'flex',
                              justifyContent: 'space-between',
                              mb: 1,
                            }}
                          >
                            <Box sx={{ flex: 1 }}>
                              <Typography variant="body2" fontWeight={500}>
                                {item.name}
                              </Typography>
                              <Typography variant="caption" color="text.secondary">
                                ₹{item.effectivePrice.toFixed(2)} each
                              </Typography>
                            </Box>
                            <IconButton
                              size="small"
                              onClick={() => removeFromCart(item.id)}
                              color="error"
                            >
                              <DeleteIcon fontSize="small" />
                            </IconButton>
                          </Box>
                          <Box
                            sx={{
                              display: 'flex',
                              justifyContent: 'space-between',
                              alignItems: 'center',
                            }}
                          >
                            <Box sx={{ display: 'flex', alignItems: 'center' }}>
                              <IconButton
                                size="small"
                                onClick={() => updateQuantity(item.id, item.quantity - 1)}
                              >
                                <RemoveIcon fontSize="small" />
                              </IconButton>
                              <TextField
                                value={item.quantity}
                                onChange={(e) => {
                                  const val = parseInt(e.target.value, 10) || 0;
                                  updateQuantity(item.id, val);
                                }}
                                type="number"
                                sx={{ width: 60, mx: 1 }}
                                size="small"
                                inputProps={{ min: 1, max: item.stockQuantity }}
                              />
                              <IconButton
                                size="small"
                                onClick={() => updateQuantity(item.id, item.quantity + 1)}
                                disabled={item.quantity >= item.stockQuantity}
                              >
                                <AddIcon fontSize="small" />
                              </IconButton>
                            </Box>
                            <Typography variant="body1" fontWeight={600}>
                              ₹{(item.effectivePrice * item.quantity).toFixed(2)}
                            </Typography>
                          </Box>
                        </Box>
                      ))}
                    </Box>
                  )}

                  <Divider sx={{ my: 2 }} />

                  <Box sx={{ display: 'flex', justifyContent: 'space-between' }}>
                    <Typography variant="h6">Total</Typography>
                    <Typography variant="h6" fontWeight={700}>
                      ₹{total.toFixed(2)}
                    </Typography>
                  </Box>
                </CardContent>
              </Card>

              <Card variant="outlined">
                <CardContent>
                  <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
                    Payment
                  </Typography>

                  <PosPaymentTiles value={paymentMethod} onChange={setPaymentMethod} />

                  <CreditEligibilityAlert
                    playerId={selectedPlayer?.id}
                    paymentMethod={paymentMethod}
                    purchaseAmount={total}
                  />

                  {paymentMethod === PaymentMethodValues.SPLIT_PAYMENT && (
                    <Grid container spacing={2} sx={{ mb: 2 }}>
                      <Grid size={{ xs: 6 }}>
                        <TextField
                          label="Cash Amount"
                          type="number"
                          value={cashAmount}
                          onChange={(e) => setCashAmount(e.target.value)}
                          fullWidth
                          InputProps={{
                            startAdornment: <InputAdornment position="start">₹</InputAdornment>,
                          }}
                        />
                      </Grid>
                      <Grid size={{ xs: 6 }}>
                        <TextField
                          label="Online Amount"
                          type="number"
                          value={onlineAmount}
                          onChange={(e) => setOnlineAmount(e.target.value)}
                          fullWidth
                          InputProps={{
                            startAdornment: <InputAdornment position="start">₹</InputAdornment>,
                          }}
                        />
                      </Grid>
                    </Grid>
                  )}

                  <TextField
                    label="Notes (Optional)"
                    value={notes}
                    onChange={(e) => setNotes(e.target.value)}
                    fullWidth
                    multiline
                    rows={2}
                    placeholder="Add any notes about this transaction..."
                    sx={{ mb: 3 }}
                  />

                  <Stack spacing={2}>
                    <Button
                      variant="contained"
                      size="large"
                      fullWidth
                      onClick={handleSubmit}
                      disabled={loading || !selectedPlayer || cart.length === 0 || creditBlocked}
                      sx={{ minHeight: 44 }}
                    >
                      {loading ? 'Processing...' : 'Complete Transaction'}
                    </Button>
                    <Button
                      variant="outlined"
                      size="large"
                      fullWidth
                      onClick={handleCancel}
                      disabled={loading}
                      sx={{ minHeight: 44 }}
                    >
                      Cancel
                    </Button>
                  </Stack>
                </CardContent>
              </Card>
            </Box>
          </Grid>
        </Grid>
      </PageShell>
    </ActiveShiftGuard>
  );
}
