import {
  Add as AddIcon,
  ShoppingCart as CartIcon,
  Delete as DeleteIcon,
  Payment as PaymentIcon,
  Person as PersonIcon,
  Remove as RemoveIcon,
} from '@mui/icons-material';
import {
  Alert,
  Autocomplete,
  Box,
  Button,
  Card,
  CardContent,
  Divider,
  GridLegacy as Grid,
  IconButton,
  InputAdornment,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import { useEffect, useState } from 'react';
import { useNavigate } from 'react-router-dom';
import {
  type PaymentMethodType,
  PaymentMethodValues,
  paymentMethodOptions,
} from '../../../containers/transactions/schemas/transaction-schema';
import { getPlayers } from '../../../services/players/list';
import { getProducts } from '../../../services/product/list';
import { addTransaction } from '../../../services/transaction/add';
import { PaymentStatus, TransactionType } from '../../../services/transaction/list';

interface Product {
  id: string;
  name: string;
  description: string;
  price: number;
  stockQuantity: number;
}

interface CartItem extends Product {
  quantity: number;
}

interface Player {
  id: string;
  username: string;
}

export default function CreateProductTransactionPage() {
  const navigate = useNavigate();
  const [error, setError] = useState<string | undefined>();
  const [success, setSuccess] = useState<string | undefined>();
  const [loading, setLoading] = useState(false);

  // Form state
  const [selectedPlayer, setSelectedPlayer] = useState<Player | null>(null);
  const [playerOptions, setPlayerOptions] = useState<Player[]>([]);
  const [playerInputValue, setPlayerInputValue] = useState('');
  const [playerLoading, setPlayerLoading] = useState(false);

  const [productOptions, setProductOptions] = useState<Product[]>([]);
  const [productInputValue, setProductInputValue] = useState('');
  const [productLoading, setProductLoading] = useState(false);

  const [cart, setCart] = useState<CartItem[]>([]);
  const [paymentMethod, setPaymentMethod] = useState<string>(PaymentMethodValues.CASH);
  const [cashAmount, setCashAmount] = useState<string>('');
  const [onlineAmount, setOnlineAmount] = useState<string>('');
  const [notes, setNotes] = useState<string>('');

  // Search players
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

  // Search products
  useEffect(() => {
    if (productInputValue.length < 2) {
      setProductOptions([]);
      return;
    }

    const searchProducts = async () => {
      setProductLoading(true);
      try {
        const data = await getProducts({
          limit: 100,
          name: productInputValue,
        });
        setProductOptions(
          data.data.map((product) => ({
            id: product.id,
            name: product.name,
            description: product.description,
            price: parseFloat(product.price),
            stockQuantity: product.stockQuantity,
          })),
        );
      } catch (_err) {
      } finally {
        setProductLoading(false);
      }
    };

    const timeoutId = setTimeout(searchProducts, 300);
    return () => clearTimeout(timeoutId);
  }, [productInputValue]);

  // Cart operations
  const addToCart = (product: Product) => {
    const existingItem = cart.find((item) => item.id === product.id);
    if (existingItem) {
      updateQuantity(product.id, existingItem.quantity + 1);
    } else {
      setCart([...cart, { ...product, quantity: 1 }]);
    }
    setProductInputValue('');
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

  // Calculations
  const subtotal = cart.reduce((sum, item) => sum + item.price * item.quantity, 0);
  const total = subtotal;

  const handleSubmit = async () => {
    setError(undefined);
    setSuccess(undefined);

    // Validation
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

    setLoading(true);

    try {
      const response = await addTransaction({
        playerId: selectedPlayer.id,
        transactionType: TransactionType.PRODUCT_PURCHASE,
        productIds: cart.map((item) => ({
          productId: item.id,
          quantity: item.quantity,
        })),
        paymentStatus: PaymentStatus.COMPLETED,
        paymentMethod: paymentMethod as PaymentMethodType,
        cashAmount:
          paymentMethod === PaymentMethodValues.SPLIT_PAYMENT ? parseFloat(cashAmount) : 0,
        onlineAmount:
          paymentMethod === PaymentMethodValues.SPLIT_PAYMENT ? parseFloat(onlineAmount) : 0,
        notes: notes || undefined,
      });

      if (response.data.transaction.paymentStatus === PaymentStatus.COMPLETED) {
        setSuccess('Transaction created successfully!');
        setTimeout(() => {
          navigate('/product-transactions');
        }, 1500);
      } else {
        setError(response.data.remarks);
      }
    } catch (err: any) {
      setError(err.message ?? 'Failed to create transaction');
    } finally {
      setLoading(false);
    }
  };

  const handleCancel = () => {
    navigate('/product-transactions');
  };

  return (
    <Box sx={{ p: 3 }}>
      <Box sx={{ mb: 3 }}>
        <Typography variant="h4" fontWeight={600} gutterBottom>
          Point of Sale - Product Transaction
        </Typography>
        <Typography variant="body2" color="text.secondary">
          Select a player, add products to cart, and complete the transaction
        </Typography>
      </Box>

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
        {/* Left Column - Player & Product Selection */}
        <Grid item xs={12} md={7}>
          {/* Player Selection */}
          <Card sx={{ mb: 3 }}>
            <CardContent>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                <PersonIcon sx={{ mr: 1, color: 'primary.main' }} />
                <Typography variant="h6" fontWeight={600}>
                  Select Player
                </Typography>
              </Box>
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
                    <Box>
                      <Typography variant="body1">{option.username}</Typography>
                    </Box>
                  </li>
                )}
              />
            </CardContent>
          </Card>

          {/* Product Search */}
          <Card>
            <CardContent>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                <CartIcon sx={{ mr: 1, color: 'primary.main' }} />
                <Typography variant="h6" fontWeight={600}>
                  Add Products
                </Typography>
              </Box>
              <Autocomplete
                options={productOptions}
                getOptionLabel={(option) => `${option.name} - ₹${option.price}`}
                inputValue={productInputValue}
                onInputChange={(_, newValue) => setProductInputValue(newValue)}
                loading={productLoading}
                onChange={(_, newValue) => {
                  if (newValue) addToCart(newValue);
                }}
                value={null}
                renderInput={(params) => (
                  <TextField {...params} placeholder="Search products to add..." fullWidth />
                )}
                renderOption={(props, option) => (
                  <li {...props} key={option.id}>
                    <Box sx={{ width: '100%' }}>
                      <Box
                        sx={{
                          display: 'flex',
                          justifyContent: 'space-between',
                          alignItems: 'center',
                        }}
                      >
                        <Box>
                          <Typography variant="body1" fontWeight={500}>
                            {option.name}
                          </Typography>
                          <Typography variant="body2" color="text.secondary">
                            {option.description}
                          </Typography>
                        </Box>
                        <Box sx={{ textAlign: 'right' }}>
                          <Typography variant="body1" fontWeight={600}>
                            ₹{option.price}
                          </Typography>
                          <Typography
                            variant="caption"
                            color={option.stockQuantity > 0 ? 'success.main' : 'error.main'}
                          >
                            {option.stockQuantity} in stock
                          </Typography>
                        </Box>
                      </Box>
                    </Box>
                  </li>
                )}
              />
            </CardContent>
          </Card>
        </Grid>

        {/* Right Column - Cart & Payment */}
        <Grid item xs={12} md={5}>
          {/* Cart */}
          <Card sx={{ mb: 3 }}>
            <CardContent>
              <Box
                sx={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  alignItems: 'center',
                  mb: 2,
                }}
              >
                <Typography variant="h6" fontWeight={600}>
                  Cart ({cart.length} items)
                </Typography>
                {cart.length > 0 && (
                  <Button size="small" color="error" onClick={clearCart} startIcon={<DeleteIcon />}>
                    Clear
                  </Button>
                )}
              </Box>

              {cart.length === 0 ? (
                <Box
                  sx={{
                    textAlign: 'center',
                    py: 4,
                    color: 'text.secondary',
                  }}
                >
                  <CartIcon sx={{ fontSize: 48, opacity: 0.3, mb: 1 }} />
                  <Typography variant="body2">Cart is empty</Typography>
                </Box>
              ) : (
                <Box sx={{ maxHeight: 400, overflowY: 'auto' }}>
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
                            ₹{item.price} each
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
                          ₹{(item.price * item.quantity).toFixed(2)}
                        </Typography>
                      </Box>
                    </Box>
                  ))}
                </Box>
              )}

              <Divider sx={{ my: 2 }} />

              <Box
                sx={{
                  display: 'flex',
                  justifyContent: 'space-between',
                  mb: 1,
                }}
              >
                <Typography variant="h6">Total</Typography>
                <Typography variant="h6" fontWeight={700}>
                  ₹{total.toFixed(2)}
                </Typography>
              </Box>
            </CardContent>
          </Card>

          {/* Payment Details */}
          <Card>
            <CardContent>
              <Box sx={{ display: 'flex', alignItems: 'center', mb: 2 }}>
                <PaymentIcon sx={{ mr: 1, color: 'primary.main' }} />
                <Typography variant="h6" fontWeight={600}>
                  Payment Details
                </Typography>
              </Box>

              <TextField
                select
                label="Payment Method"
                value={paymentMethod}
                onChange={(e) => setPaymentMethod(e.target.value)}
                fullWidth
                sx={{ mb: 2 }}
              >
                {paymentMethodOptions.map((option) => (
                  <MenuItem key={option.value} value={option.value}>
                    {option.label}
                  </MenuItem>
                ))}
              </TextField>

              {paymentMethod === PaymentMethodValues.SPLIT_PAYMENT && (
                <Grid container spacing={2} sx={{ mb: 2 }}>
                  <Grid item xs={6}>
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
                  <Grid item xs={6}>
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
                  disabled={loading || !selectedPlayer || cart.length === 0}
                >
                  {loading ? 'Processing...' : 'Complete Transaction'}
                </Button>
                <Button
                  variant="outlined"
                  size="large"
                  fullWidth
                  onClick={handleCancel}
                  disabled={loading}
                >
                  Cancel
                </Button>
              </Stack>
            </CardContent>
          </Card>
        </Grid>
      </Grid>
    </Box>
  );
}
