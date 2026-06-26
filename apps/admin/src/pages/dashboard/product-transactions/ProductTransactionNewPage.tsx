import { FormButton, IntegerField } from '@gaming-cafe/ui';
import { useAsyncAction } from '@gaming-cafe/utils';
import {
  Add as AddIcon,
  ShoppingCart as CartIcon,
  Delete as DeleteIcon,
  Remove as RemoveIcon,
  Search as SearchIcon,
} from '@mui/icons-material';
import {
  Alert,
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
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { ActiveShiftGuard } from '../../../components/ActiveShiftGuard';
import CreditEligibilityAlert from '../../../components/CreditEligibilityAlert';
import {
  CounterSaleLayout,
  evaluateCreditBlocked,
  isPosPaymentSuccessful,
  PosPaymentTiles,
  type PosPlayer,
  PosPlayerPicker,
  PosSplitAmountFields,
  PosStoreToolbar,
  posSaleSuccessLabel,
  validateSplitPaymentAmounts,
} from '../../../containers/sales';
import {
  type PaymentMethodType,
  PaymentMethodValues,
} from '../../../containers/transactions/schemas/transaction-schema';
import { getPlayerCredit } from '../../../services/credit';
import { getInventoryLocations, getLocationStock } from '../../../services/inventory';
import { getKioskOrder } from '../../../services/kiosk-orders';
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

function PosProductCard({
  product,
  cartQuantity,
  nightActive,
  onAdd,
  disabled = false,
}: {
  product: Product;
  cartQuantity: number;
  nightActive: boolean;
  onAdd: (product: Product) => void;
  disabled?: boolean;
}) {
  const inCart = cartQuantity > 0;
  const outOfStock = product.stockQuantity <= 0;

  return (
    <Card
      variant="outlined"
      sx={{
        height: '100%',
        opacity: outOfStock || disabled ? 0.5 : 1,
        pointerEvents: outOfStock || disabled ? 'none' : 'auto',
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

export default function CreateProductTransactionPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const prefillOrderId = searchParams.get('orderId') ?? undefined;
  const [error, setError] = useState<string | undefined>();
  const {
    loading: submitting,
    succeeded,
    failed,
    errorMessage,
    disabled: submitDisabled,
    run,
    clearError,
  } = useAsyncAction({ throttleMs: 1000, lockOnSuccess: true });

  const [selectedPlayer, setSelectedPlayer] = useState<PosPlayer | null>(null);
  const [productSearch, setProductSearch] = useState('');
  const [cart, setCart] = useState<CartItem[]>([]);
  const [paymentMethod, setPaymentMethod] = useState<string>(PaymentMethodValues.CASH);
  const [cashAmount, setCashAmount] = useState<string>('');
  const [onlineAmount, setOnlineAmount] = useState<string>('');
  const [notes, setNotes] = useState<string>('');
  const [saleLocationId, setSaleLocationId] = useState<string>('');
  const [kioskOrderId, setKioskOrderId] = useState<string | undefined>(prefillOrderId);

  const { data: prefillOrder } = useQuery({
    queryKey: ['kiosk-order', prefillOrderId],
    queryFn: () => getKioskOrder(prefillOrderId as string),
    enabled: !!prefillOrderId,
  });

  useEffect(() => {
    if (!prefillOrder) return;
    setKioskOrderId(prefillOrder.id);
    setSelectedPlayer({
      id: prefillOrder.playerId,
      username: prefillOrder.playerUsername ?? prefillOrder.playerId,
    });
    if (prefillOrder.playerNote) {
      setNotes(prefillOrder.playerNote);
    }
    setCart(
      prefillOrder.lineItems.map((item) => ({
        id: item.productId,
        name: item.productName,
        description: '',
        price: item.unitPrice,
        dayPrice: item.unitPrice,
        nightPrice: item.unitPrice,
        stockQuantity: item.quantity,
        unitsPerPurchaseUnit: 1,
        quantity: item.quantity,
        effectivePrice: item.unitPrice,
      })),
    );
  }, [prefillOrder]);

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

  const checkoutErrorClearKey = [
    selectedPlayer?.id,
    cart.length,
    cart.map((item) => `${item.id}:${item.quantity}`).join(','),
    paymentMethod,
    cashAmount,
    onlineAmount,
    saleLocationId,
  ].join('|');

  // biome-ignore lint/correctness/useExhaustiveDependencies: clear API error when checkout inputs change after failure
  useEffect(() => {
    if (failed) clearError();
  }, [checkoutErrorClearKey, failed, clearError]);

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
  const { data: creditDetail, isFetching: creditLoading } = useQuery({
    queryKey: ['player-credit', selectedPlayer?.id],
    queryFn: () => {
      if (!selectedPlayer) throw new Error('No player selected');
      return getPlayerCredit(selectedPlayer.id);
    },
    enabled: isCredit && !!selectedPlayer,
  });

  const creditBlocked = useMemo(
    () => evaluateCreditBlocked(isCredit, total, creditDetail, creditLoading),
    [isCredit, total, creditDetail, creditLoading],
  );

  const handleSubmit = () => {
    setError(undefined);

    if (!selectedPlayer) {
      setError('Please select a player');
      return;
    }

    if (cart.length === 0) {
      setError('Please add at least one product to the cart');
      return;
    }

    if (paymentMethod === PaymentMethodValues.SPLIT_PAYMENT) {
      const splitError = validateSplitPaymentAmounts(total, cashAmount, onlineAmount);
      if (splitError) {
        setError(splitError);
        return;
      }
    }

    if (isCredit && creditBlocked) {
      setError('This player is not eligible for this credit purchase');
      return;
    }

    if (!saleLocationId) {
      setError('Please select a store location');
      return;
    }

    void run(async () => {
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
          kioskOrderId,
          lineItems: cart.map((item) => ({
            productId: item.id,
            quantity: item.quantity,
            unitPrice: item.effectivePrice,
          })),
        });

        if (!isPosPaymentSuccessful(response.paymentStatus)) {
          throw new Error('Payment not completed');
        }

        setTimeout(() => {
          navigate('/product-transactions');
        }, 1500);
      } catch (err: unknown) {
        throw err instanceof Error ? err : new Error('Failed to create transaction');
      }
    });
  };

  const handleCancel = () => {
    navigate('/product-transactions');
  };

  const handleLocationChange = (id: string) => {
    setSaleLocationId(id);
    if (!kioskOrderId) {
      setCart([]);
    }
  };

  const alerts = (
    <>
      {prefillOrder ? (
        <Alert severity="info" sx={{ mb: 3 }}>
          Converting kiosk order from {prefillOrder.deviceName ?? 'station'} —{' '}
          {prefillOrder.playerUsername ?? 'player'}. Choose payment method and complete the sale.
        </Alert>
      ) : null}
      {error && (
        <Alert severity="error" sx={{ mb: 3 }} onClose={() => setError(undefined)}>
          {error}
        </Alert>
      )}
    </>
  );

  const catalog = (
    <>
      <PosPlayerPicker value={selectedPlayer} onChange={setSelectedPlayer} disabled={submitting} />

      <Typography variant="subtitle1" fontWeight={600} sx={{ mb: 2 }}>
        Products
      </Typography>
      <TextField
        placeholder="Search products..."
        value={productSearch}
        onChange={(e) => setProductSearch(e.target.value)}
        fullWidth
        size="small"
        disabled={submitting}
        sx={{ mb: 2 }}
        helperText="Filter the product grid by name"
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
        <Typography variant="body2" color="text.secondary" sx={{ py: 4, textAlign: 'center' }}>
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
                disabled={submitting}
              />
            </Grid>
          ))}
        </Grid>
      )}
    </>
  );

  const summary = (
    <>
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
              <Button size="small" color="error" onClick={clearCart} startIcon={<DeleteIcon />}>
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
                      disabled={submitting}
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
                        disabled={submitting}
                      >
                        <RemoveIcon fontSize="small" />
                      </IconButton>
                      <IntegerField
                        value={item.quantity}
                        onChange={(e) => {
                          const val = Number.parseInt(e.target.value, 10) || 0;
                          updateQuantity(item.id, val);
                        }}
                        sx={{ width: 60, mx: 1 }}
                        size="small"
                        disabled={submitting}
                        inputProps={{ min: 1, max: item.stockQuantity }}
                      />
                      <IconButton
                        size="small"
                        onClick={() => updateQuantity(item.id, item.quantity + 1)}
                        disabled={submitting || item.quantity >= item.stockQuantity}
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

          <PosPaymentTiles
            value={paymentMethod}
            onChange={setPaymentMethod}
            disabled={submitting}
          />

          <CreditEligibilityAlert
            playerId={selectedPlayer?.id}
            paymentMethod={paymentMethod}
            purchaseAmount={total}
          />

          {paymentMethod === PaymentMethodValues.SPLIT_PAYMENT && (
            <PosSplitAmountFields
              cashAmount={cashAmount}
              onlineAmount={onlineAmount}
              onCashChange={setCashAmount}
              onOnlineChange={setOnlineAmount}
              totalAmount={total}
            />
          )}

          <TextField
            label="Notes (Optional)"
            value={notes}
            onChange={(e) => setNotes(e.target.value)}
            fullWidth
            multiline
            rows={2}
            disabled={submitting}
            placeholder="Add any notes about this transaction..."
            helperText="Optional staff note stored on the transaction (max 500 chars)"
            sx={{ mb: 3 }}
          />

          <Stack spacing={2}>
            <FormButton
              variant="contained"
              size="large"
              fullWidth
              onClick={handleSubmit}
              loading={submitting}
              success={succeeded}
              successLabel={posSaleSuccessLabel(paymentMethod)}
              error={failed}
              errorLabel={errorMessage ?? 'Failed to create transaction'}
              disabled={!selectedPlayer || cart.length === 0 || creditBlocked || submitDisabled}
              sx={{ minHeight: 44 }}
            >
              Complete sale
            </FormButton>
            <Button
              variant="outlined"
              size="large"
              fullWidth
              onClick={handleCancel}
              disabled={submitting}
              sx={{ minHeight: 44 }}
            >
              Cancel
            </Button>
          </Stack>
        </CardContent>
      </Card>
    </>
  );

  return (
    <ActiveShiftGuard>
      <CounterSaleLayout
        backTo="/product-transactions"
        backLabel="POS sales"
        toolbar={
          <PosStoreToolbar
            saleLocationId={saleLocationId}
            storeLocations={storeLocations}
            nightActive={nightActive}
            onLocationChange={handleLocationChange}
          />
        }
        alerts={alerts}
        catalog={catalog}
        summary={summary}
      />
    </ActiveShiftGuard>
  );
}
