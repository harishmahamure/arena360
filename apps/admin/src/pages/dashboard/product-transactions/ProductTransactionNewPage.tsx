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
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { ActiveShiftGuard } from '../../../components/ActiveShiftGuard';
import CreditEligibilityAlert from '../../../components/CreditEligibilityAlert';
import {
  CounterSaleLayout,
  evaluateCreditBlocked,
  isPosPaymentSuccessful,
  PosOnlinePaymentRefField,
  PosPaymentTiles,
  type PosPlayer,
  PosPlayerPicker,
  PosSplitAmountFields,
  PosStoreToolbar,
  posSaleSuccessLabel,
  requiresOnlinePaymentRef,
  validateOnlinePaymentRefLast4,
  validateSplitPaymentAmounts,
} from '../../../containers/sales';
import { type PickedOptions, PosOptionPicker } from '../../../containers/sales/PosOptionPicker';
import {
  type PaymentMethodType,
  PaymentMethodValues,
} from '../../../containers/transactions/schemas/transaction-schema';
import { getPlayerCredit } from '../../../services/credit';
import { getInventoryLocations, getLocationStock } from '../../../services/inventory';
import { getKioskOrder } from '../../../services/kiosk-orders';
import { getProducts, type ProductResponse } from '../../../services/product/list';
import {
  getCurrentProductPrices,
  getProductRecipe,
  type ProductCurrentPrice,
  type ProductOptionGroup,
} from '../../../services/product/recipe';
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
  hasOptions: boolean;
  madeToOrder: boolean;
}

interface CartItem extends Product {
  /** Product plus chosen options, so the same burger with different options gets its own line. */
  lineKey: string;
  optionIds: string[];
  optionNames: string[];
  quantity: number;
  effectivePrice: number;
}

function mapProductForPos(
  product: ProductResponse,
  stockByProduct: Map<string, number>,
  currentPrice: ProductCurrentPrice | undefined,
): Product {
  const dayPrice = product.dayPrice ?? parseFloat(product.price);
  const nightPrice = product.nightPrice ?? dayPrice;
  const madeToOrder = currentPrice?.madeToOrderAvailable != null;
  const stock = madeToOrder
    ? (currentPrice?.madeToOrderAvailable ?? 0)
    : (stockByProduct.get(product.id) ?? product.stockQuantity ?? 0);
  return {
    id: product.id,
    name: product.name,
    description: product.description,
    price: currentPrice?.price ?? effectiveProductPrice(dayPrice, nightPrice),
    dayPrice,
    nightPrice,
    stockQuantity: stock,
    unitsPerPurchaseUnit: product.unitsPerPurchaseUnit ?? 1,
    hasOptions: currentPrice?.hasOptions ?? false,
    madeToOrder,
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
            {product.madeToOrder
              ? `${product.stockQuantity} can be made`
              : `${product.stockQuantity} pcs in store`}
          </Typography>
          {product.hasOptions && (
            <Typography variant="caption" display="block" color="text.secondary">
              Has options
            </Typography>
          )}
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
  const [onlinePaymentRefLast4, setOnlinePaymentRefLast4] = useState<string>('');
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
        lineKey: item.productId,
        optionIds: [],
        optionNames: [],
        name: item.productName,
        description: '',
        price: item.unitPrice,
        dayPrice: item.unitPrice,
        nightPrice: item.unitPrice,
        stockQuantity: item.quantity,
        unitsPerPurchaseUnit: 1,
        hasOptions: false,
        madeToOrder: false,
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
    cart.map((item) => `${item.lineKey}:${item.quantity}`).join(','),
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
    queryFn: () => getProducts({ forSale: true, limit: 200, sortBy: 'name', sortOrder: 'ASC' }),
    enabled: !!saleLocationId,
  });

  const { data: currentPrices } = useQuery({
    queryKey: ['pos-current-prices', saleLocationId],
    queryFn: () => getCurrentProductPrices(saleLocationId),
    enabled: !!saleLocationId,
    refetchInterval: 60_000,
  });

  const catalogProducts = useMemo(() => {
    const priceByProduct = new Map((currentPrices ?? []).map((row) => [row.productId, row]));
    return (productsData?.data ?? [])
      .filter((p) => p.isActive)
      .map((product) => mapProductForPos(product, stockByProduct, priceByProduct.get(product.id)));
  }, [productsData, stockByProduct, currentPrices]);

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
      map.set(item.id, (map.get(item.id) ?? 0) + item.quantity);
    }
    return map;
  }, [cart]);

  const queryClient = useQueryClient();
  const [optionTarget, setOptionTarget] = useState<{
    product: Product;
    groups: ProductOptionGroup[];
  } | null>(null);

  const addLine = (product: Product, picked: PickedOptions) => {
    const lineKey = [product.id, ...[...picked.optionIds].sort()].join('|');
    const existingItem = cart.find((item) => item.lineKey === lineKey);
    if (existingItem) {
      updateQuantity(lineKey, existingItem.quantity + 1);
      return;
    }
    if ((cartQtyByProduct.get(product.id) ?? 0) + 1 > product.stockQuantity) {
      setError(`Only ${product.stockQuantity} units available in stock`);
      return;
    }
    const effectivePrice = Math.round(Math.max(0, product.price + picked.priceDelta) * 100) / 100;
    setCart([
      ...cart,
      {
        ...product,
        lineKey,
        optionIds: picked.optionIds,
        optionNames: picked.names,
        price: effectivePrice,
        effectivePrice,
        quantity: 1,
      },
    ]);
  };

  const addToCart = async (product: Product) => {
    if (!product.hasOptions) {
      addLine(product, { optionIds: [], names: [], priceDelta: 0 });
      return;
    }
    try {
      const recipe = await queryClient.fetchQuery({
        queryKey: ['product-recipe', product.id],
        queryFn: () => getProductRecipe(product.id),
        staleTime: 60_000,
      });
      setOptionTarget({ product, groups: recipe.optionGroups });
    } catch {
      setError(`Could not load options for ${product.name}`);
    }
  };

  const updateQuantity = (lineKey: string, newQuantity: number) => {
    const item = cart.find((item) => item.lineKey === lineKey);
    if (!item) return;

    if (newQuantity <= 0) {
      removeFromCart(lineKey);
      return;
    }

    const otherLines = (cartQtyByProduct.get(item.id) ?? 0) - item.quantity;
    if (otherLines + newQuantity > item.stockQuantity) {
      setError(`Only ${item.stockQuantity} units available in stock`);
      return;
    }

    setCart(
      cart.map((item) => (item.lineKey === lineKey ? { ...item, quantity: newQuantity } : item)),
    );
  };

  const removeFromCart = (lineKey: string) => {
    setCart(cart.filter((item) => item.lineKey !== lineKey));
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

    const onlinePortion =
      paymentMethod === PaymentMethodValues.SPLIT_PAYMENT
        ? Number.parseFloat(onlineAmount)
        : paymentMethod === PaymentMethodValues.ONLINE
          ? total
          : undefined;
    const refError = validateOnlinePaymentRefLast4(
      onlinePaymentRefLast4,
      paymentMethod,
      onlinePortion,
    );
    if (refError) {
      setError(refError);
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
          onlinePaymentRefLast4: requiresOnlinePaymentRef(paymentMethod, onlinePortion)
            ? onlinePaymentRefLast4.trim()
            : undefined,
          notes: notes || undefined,
          kioskOrderId,
          lineItems: cart.map((item) => ({
            productId: item.id,
            quantity: item.quantity,
            unitPrice: item.effectivePrice,
            optionIds: item.optionIds.length ? item.optionIds : undefined,
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

      <Stack direction="row" spacing={1.5} alignItems="center" sx={{ mb: 2 }}>
        <TextField
          placeholder="Search products"
          aria-label="Search products"
          value={productSearch}
          onChange={(e) => setProductSearch(e.target.value)}
          fullWidth
          disabled={submitting}
          slotProps={{
            htmlInput: { 'data-autofocus': true },
            input: {
              startAdornment: (
                <InputAdornment position="start">
                  <SearchIcon fontSize="small" color="action" />
                </InputAdornment>
              ),
            },
          }}
        />
        <PosStoreToolbar
          saleLocationId={saleLocationId}
          storeLocations={storeLocations}
          nightActive={nightActive}
          onLocationChange={handleLocationChange}
        />
      </Stack>

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
                  key={item.lineKey}
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
                      {item.optionNames.length > 0 && (
                        <Typography variant="caption" color="text.secondary" display="block">
                          {item.optionNames.join(', ')}
                        </Typography>
                      )}
                      <Typography variant="caption" color="text.secondary">
                        ₹{item.effectivePrice.toFixed(2)} each
                      </Typography>
                    </Box>
                    <IconButton
                      size="small"
                      onClick={() => removeFromCart(item.lineKey)}
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
                        onClick={() => updateQuantity(item.lineKey, item.quantity - 1)}
                        disabled={submitting}
                      >
                        <RemoveIcon fontSize="small" />
                      </IconButton>
                      <IntegerField
                        value={item.quantity}
                        onChange={(e) => {
                          const val = Number.parseInt(e.target.value, 10) || 0;
                          updateQuantity(item.lineKey, val);
                        }}
                        sx={{ width: 60, mx: 1 }}
                        size="small"
                        disabled={submitting}
                        inputProps={{ min: 1, max: item.stockQuantity }}
                      />
                      <IconButton
                        size="small"
                        onClick={() => updateQuantity(item.lineKey, item.quantity + 1)}
                        disabled={
                          submitting || (cartQtyByProduct.get(item.id) ?? 0) >= item.stockQuantity
                        }
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

          {requiresOnlinePaymentRef(
            paymentMethod,
            paymentMethod === PaymentMethodValues.SPLIT_PAYMENT ? onlineAmount : total,
          ) && (
            <PosOnlinePaymentRefField
              value={onlinePaymentRefLast4}
              onChange={setOnlinePaymentRefLast4}
              disabled={submitting}
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
        </CardContent>
      </Card>
    </>
  );

  return (
    <ActiveShiftGuard>
      <PosOptionPicker
        productName={optionTarget?.product.name ?? ''}
        basePrice={optionTarget?.product.price ?? 0}
        groups={optionTarget?.groups ?? null}
        onCancel={() => setOptionTarget(null)}
        onConfirm={(picked) => {
          if (optionTarget) addLine(optionTarget.product, picked);
          setOptionTarget(null);
        }}
      />
      <CounterSaleLayout
        title="New sale"
        exitTo="/product-transactions"
        alerts={alerts}
        catalog={catalog}
        summary={summary}
        actions={
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
        }
        busy={submitting || succeeded}
      />
    </ActiveShiftGuard>
  );
}
