import { type ErrorCode, getErrorMessage, isErrorCode } from '@gaming-cafe/contracts';
import { ApiError } from '@gaming-cafe/utils';
import { useMemo, useState } from 'react';
import { type KioskMenuProduct, useKioskOrder, useKioskProducts } from '../../hooks/useKioskMenu';

interface CartLine {
  product: KioskMenuProduct;
  quantity: number;
}

interface MenuViewProps {
  onError?: (message: string) => void;
}

const CATEGORY_LABELS: Record<string, string> = {
  beverage: 'Drinks',
  snack: 'Snacks',
  meal: 'Meals',
  other: 'Other',
};

function formatPrice(amount: number): string {
  return `₹${amount.toFixed(0)}`;
}

function orderStatusLabel(status: string): string {
  switch (status) {
    case 'pending':
      return 'Order sent — staff is preparing your items';
    case 'preparing':
      return 'Your order is being prepared';
    case 'fulfilled':
      return 'Order delivered — enjoy!';
    case 'cancelled':
      return 'Order was cancelled — ask staff if you still need items';
    default:
      return status;
  }
}

export function MenuView({ onError }: MenuViewProps) {
  const { products, loading, error } = useKioskProducts();
  const { currentOrder, submitting, submitOrder } = useKioskOrder();
  const [category, setCategory] = useState<string | null>(null);
  const [cart, setCart] = useState<CartLine[]>([]);
  const [note, setNote] = useState('');
  const [success, setSuccess] = useState(false);

  const categories = useMemo(() => {
    const set = new Set(products.map((p) => p.category));
    return Array.from(set).sort();
  }, [products]);

  const visible = useMemo(() => {
    if (!category) return products;
    return products.filter((p) => p.category === category);
  }, [products, category]);

  const cartTotal = useMemo(
    () => cart.reduce((sum, line) => sum + line.product.price * line.quantity, 0),
    [cart],
  );

  const hasOpenOrder =
    currentOrder != null &&
    (currentOrder.status === 'pending' || currentOrder.status === 'preparing');

  function addToCart(product: KioskMenuProduct) {
    if (!product.inStock || hasOpenOrder) return;
    setCart((prev) => {
      const existing = prev.find((l) => l.product.id === product.id);
      if (existing) {
        return prev.map((l) =>
          l.product.id === product.id ? { ...l, quantity: l.quantity + 1 } : l,
        );
      }
      return [...prev, { product, quantity: 1 }];
    });
  }

  function changeQty(productId: string, delta: number) {
    setCart((prev) =>
      prev
        .map((l) => (l.product.id === productId ? { ...l, quantity: l.quantity + delta } : l))
        .filter((l) => l.quantity > 0),
    );
  }

  async function handleSubmit() {
    if (cart.length === 0 || hasOpenOrder) return;
    setSuccess(false);
    try {
      await submitOrder(
        cart.map((l) => ({ productId: l.product.id, quantity: l.quantity })),
        note.trim() || undefined,
      );
      setCart([]);
      setNote('');
      setSuccess(true);
    } catch (e) {
      const msg =
        e instanceof ApiError && isErrorCode(e.message)
          ? getErrorMessage(e.message as ErrorCode)
          : e instanceof Error
            ? e.message
            : 'Could not send order';
      onError?.(msg);
    }
  }

  if (loading) {
    return (
      <section className="a360-section">
        <p className="a360-muted">Loading menu…</p>
      </section>
    );
  }

  if (error) {
    return (
      <section className="a360-section">
        <p className="a360-error">{error}</p>
      </section>
    );
  }

  return (
    <section className="a360-section a360-menu">
      <header className="a360-library-head">
        <div>
          <h1 className="a360-library-title">
            Cafe <span>/ Menu</span>
          </h1>
          <p className="a360-library-sub">
            Order snacks and drinks — staff will bring items to your station.
          </p>
        </div>
      </header>

      {currentOrder ? (
        <div className={`a360-menu-order-status status-${currentOrder.status}`} role="status">
          <span className="material-symbols-outlined">receipt_long</span>
          <div>
            <strong>{orderStatusLabel(currentOrder.status)}</strong>
            <p className="a360-muted">
              {currentOrder.lineItems.map((i) => `${i.quantity}× ${i.productName}`).join(', ')}
            </p>
          </div>
        </div>
      ) : null}

      {success && !hasOpenOrder ? (
        <div className="a360-menu-success" role="status">
          Order sent — staff will bring items to your station.
        </div>
      ) : null}

      <div className="a360-menu-layout">
        <div className="a360-menu-catalog">
          {categories.length > 1 ? (
            <div className="a360-menu-categories">
              <button
                type="button"
                className={`a360-chip${category === null ? ' is-active' : ''}`}
                onClick={() => setCategory(null)}
              >
                All
              </button>
              {categories.map((cat) => (
                <button
                  key={cat}
                  type="button"
                  className={`a360-chip${category === cat ? ' is-active' : ''}`}
                  onClick={() => setCategory(cat)}
                >
                  {CATEGORY_LABELS[cat] ?? cat}
                </button>
              ))}
            </div>
          ) : null}

          <div className="a360-menu-grid">
            {visible.map((product) => (
              <button
                key={product.id}
                type="button"
                className={`a360-menu-card${product.inStock ? '' : ' is-disabled'}`}
                disabled={!product.inStock || hasOpenOrder}
                onClick={() => addToCart(product)}
              >
                <span className="a360-menu-card-cat">
                  {CATEGORY_LABELS[product.category] ?? product.category}
                </span>
                <span className="a360-menu-card-name">{product.name}</span>
                {product.description ? (
                  <span className="a360-menu-card-desc">{product.description}</span>
                ) : null}
                <span className="a360-menu-card-price">{formatPrice(product.price)}</span>
                {!product.inStock ? <span className="a360-menu-card-oos">Out of stock</span> : null}
              </button>
            ))}
          </div>
        </div>

        <aside className="a360-menu-cart">
          <h2 className="a360-menu-cart-title">Your order</h2>
          {cart.length === 0 ? (
            <p className="a360-muted">Tap items to add them here.</p>
          ) : (
            <ul className="a360-menu-cart-lines">
              {cart.map((line) => (
                <li key={line.product.id} className="a360-menu-cart-line">
                  <div>
                    <span className="a360-menu-cart-name">{line.product.name}</span>
                    <span className="a360-muted">{formatPrice(line.product.price)} each</span>
                  </div>
                  <div className="a360-menu-cart-qty">
                    <button
                      type="button"
                      aria-label="Decrease"
                      onClick={() => changeQty(line.product.id, -1)}
                    >
                      −
                    </button>
                    <span>{line.quantity}</span>
                    <button
                      type="button"
                      aria-label="Increase"
                      onClick={() => changeQty(line.product.id, 1)}
                    >
                      +
                    </button>
                  </div>
                </li>
              ))}
            </ul>
          )}

          {cart.length > 0 ? (
            <>
              <p className="a360-menu-cart-total">
                Total <strong>{formatPrice(cartTotal)}</strong>
              </p>
              <p className="a360-muted a360-menu-pay-note">Pay at counter when staff delivers.</p>
              <label className="a360-menu-note">
                <span>Note for staff (optional)</span>
                <input
                  type="text"
                  value={note}
                  maxLength={200}
                  placeholder="e.g. extra ice"
                  onChange={(e) => setNote(e.target.value)}
                />
              </label>
              <button
                type="button"
                className="a360-btn-primary a360-menu-submit"
                disabled={submitting || hasOpenOrder}
                onClick={() => void handleSubmit()}
              >
                {submitting ? 'Sending…' : 'Send order to staff'}
              </button>
            </>
          ) : null}

          {hasOpenOrder ? (
            <p className="a360-muted">Wait for your current order before placing another.</p>
          ) : null}
        </aside>
      </div>
    </section>
  );
}
