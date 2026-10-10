// Immutable M0 input generator from commit 54618df; fixed date matches the committed golden reports.
/** Synthetic, repeatable reporting demo. No application credentials are created. */
import { createHash } from 'node:crypto';

const DAY = 86_400_000;
const PREFIX = 'arena360-demo-v1';
export function demoId(key) {
  const hex = createHash('sha256').update(`${PREFIX}:${key}`).digest('hex');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-4${hex.slice(13, 16)}-a${hex.slice(17, 20)}-${hex.slice(20, 32)}`;
}

export function buildDemo(end = new Date()) {
  if (!Number.isFinite(end.getTime())) throw new Error('Invalid demo date');
  const lastDay = Date.UTC(end.getUTCFullYear(), end.getUTCMonth(), end.getUTCDate());
  const firstDay = lastDay - 59 * DAY;
  const tables = {};
  const add = (table, key, values) => {
    const row = { id: demoId(key), ...values };
    tables[table] ??= [];
    tables[table].push(row);
    return row.id;
  };
  // Last-day activities finish before the seed timestamp, even early in the morning.
  const at = (day, hour = 12) =>
    new Date(
      Math.min(firstDay + day * DAY + hour * 3_600_000, end.getTime() - 300_000),
    ).toISOString();
  const stamp = (createdAt) => ({ createdAt, updatedAt: createdAt });
  const staff = add('users', 'staff', {
    username: 'demo.counter',
    firstName: 'Demo',
    lastName: 'Counter',
    role: 'staff',
    password_hash: '!demo-login-disabled!',
    isActive: true,
    ...stamp(at(0, 0)),
  });
  const names = ['Aarav', 'Maya', 'Kabir', 'Isha', 'Rohan', 'Tara', 'Arjun', 'Nia'];
  const players = Array.from({ length: 32 }, (_, i) =>
    add('users', `player-${i}`, {
      username: `demo.player.${String(i + 1).padStart(2, '0')}`,
      firstName: names[i % names.length],
      lastName: `Demo ${i + 1}`,
      role: 'player',
      password_hash: '!demo-login-disabled!',
      isActive: true,
      creditLimit: '5000.0000',
      ...stamp(at(i < 24 ? 0 : 35 + i - 24, 0)),
    }),
  );
  const devices = Array.from({ length: 12 }, (_, i) =>
    add('devices', `device-${i}`, {
      name: `DEMO PC ${String(i + 1).padStart(2, '0')}`,
      deviceType: 'PC',
      deviceSubType: 'HIGH_END_PCS',
      status: i === 11 ? 'under_maintenance' : 'available',
      registrationStatus: 'unregistered',
      location: 'Demo gaming floor',
      ...stamp(at(0, 0)),
    }),
  );
  const plans = [
    ['Quick Play', 60, 120],
    ['Ranked Duo', 120, 220],
    ['Squad Marathon', 180, 300],
  ].map(([name, minutes, price], i) => ({
    id: add('plans', `plan-${i}`, {
      name: `DEMO ${name}`,
      description: 'Synthetic demo plan',
      price,
      planType: 'time_based',
      durationMinutes: minutes,
      timeCredits: minutes,
      perMinuteRate: 2,
      validityDays: 30,
      deviceType: 'PC',
      deviceSubType: 'HIGH_END_PCS',
      ...stamp(at(0, 0)),
    }),
    minutes,
    price,
  }));
  const products = [
    ['Cola', 60, 30, 'beverage'],
    ['Energy Drink', 140, 80, 'beverage'],
    ['Chips', 50, 25, 'snack'],
    ['Sandwich', 120, 65, 'meal'],
    ['Water', 30, 12, 'beverage'],
  ].map(([name, price, cost, category], i) => ({
    id: add('products', `product-${i}`, {
      name: `DEMO ${name}`,
      sku: `DEMO-${i + 1}`,
      price,
      dayPrice: price,
      nightPrice: price,
      purchasePrice: cost,
      purchasePricePerBox: cost * 12,
      unitsPerPurchaseUnit: 12,
      category,
      ...stamp(at(0, 0)),
    }),
    price,
    cost,
    sold: 0,
    waste: 0,
  }));
  const vendor = add('vendors', 'vendor', {
    name: 'DEMO Refreshment Supply',
    notes: 'Synthetic supplier',
    ...stamp(at(0, 0)),
  });
  const location = add('inventory_locations', 'location', {
    name: 'DEMO Cafe Counter',
    kind: 'store',
    ...stamp(at(0, 0)),
  });
  const categories = ['Utilities', 'Consumables', 'Maintenance'].map((name, i) =>
    add('expense_categories', `category-${i}`, {
      name: `DEMO ${name}`,
      budgetAmount: [18000, 12000, 8000][i],
      budgetPeriod: 'monthly',
      ...stamp(at(0, 0)),
    }),
  );
  for (let day = 0; day < 60; day++) {
    const shift = add('shifts', `shift-${day}`, {
      userId: staff,
      clockIn: at(day, 0),
      clockOut: day === 59 ? null : at(day, 20),
      status: day === 59 ? 'active' : 'completed',
      notes: 'Demo shift',
      ...stamp(at(day, 3)),
    });
    const registerId = demoId(`register-${day}`);
    let cash = 0;
    const entry = (key, entryType, amount, referenceId, referenceType, createdAt) =>
      add('cash_register_entries', key, {
        cashRegisterId: registerId,
        entryType,
        amount,
        referenceId,
        referenceType,
        createdBy: staff,
        createdAt,
        reason: 'Synthetic demo activity',
      });
    const weekend = [0, 6].includes(new Date(firstDay + day * DAY).getUTCDay());
    const visits = 6 + Math.floor(day / 10) + (weekend ? 4 : 0);
    for (let visit = 0; visit < visits; visit++) {
      const n = day * 20 + visit;
      const player = players[(day + visit * 3) % (day < 35 ? 24 : Math.min(32, 25 + day - 35))];
      const plan = plans[n % plans.length];
      const isPlan = visit % 2 === 0;
      const product = products[n % products.length];
      const quantity = 1 + (n % 3);
      const amount = isPlan ? plan.price : product.price * quantity;
      const status =
        n % 37 === 0
          ? 'refunded'
          : n % 31 === 0
            ? 'failed'
            : n % 29 === 0
              ? 'pending'
              : 'completed';
      const method = ['cash', 'online', 'split_payment', 'credit'][(day + visit) % 4];
      const booked = status === 'completed';
      const paid = booked && method === 'credit' ? amount / 2 : 0;
      const time = at(day, 5 + visit * 0.7);
      const transaction = add('transactions', `sale-${n}`, {
        playerId: player,
        planId: isPlan ? plan.id : null,
        amount,
        transactionType: isPlan ? 'plan_purchase' : 'product_purchase',
        paymentMethod: method,
        paymentStatus: booked && method === 'credit' ? 'credit' : status,
        cashAmount: method === 'split_payment' ? amount / 2 : null,
        onlineAmount: method === 'split_payment' ? amount / 2 : null,
        paidAmount: paid,
        transactionDate: time,
        shiftId: shift,
        notes: 'DEMO synthetic sale',
        createdBy: staff,
        ...stamp(time),
      });
      const cashPaid = booked
        ? method === 'cash'
          ? amount
          : method === 'split_payment'
            ? amount / 2
            : 0
        : 0;
      if (cashPaid) {
        cash += cashPaid;
        entry(`sale-cash-${n}`, 'cash_in', cashPaid, transaction, 'transaction', time);
      }
      if (!isPlan) {
        add('transaction_products', `sale-product-${n}`, {
          transactionId: transaction,
          productId: product.id,
          quantity,
          unitPrice: product.price,
          priceAtPurchase: product.price,
          subtotal: amount,
          createdAt: time,
        });
        if (booked) {
          product.sold += quantity;
          add('stock_movements', `sale-stock-${n}`, {
            locationId: location,
            productId: product.id,
            delta: -quantity,
            movementType: 'sale',
            referenceId: transaction,
            referenceType: 'transaction',
            createdBy: staff,
            createdAt: time,
          });
        }
      }
      if (isPlan && booked) {
        const minutes = plan.minutes;
        const live = day === 59 && visit < 3;
        const used = live ? 0 : minutes;
        const sessionStart = new Date(
          live ? end.getTime() - 20 * 60_000 : new Date(time).getTime() - minutes * 60_000,
        ).toISOString();
        // Purchase precedes usage; an exhausted wallet has a balanced purchase/usage ledger.
        tables.transactions.at(-1).transactionDate = sessionStart;
        tables.transactions.at(-1).createdAt = sessionStart;
        tables.transactions.at(-1).updatedAt = sessionStart;
        const expiry = new Date(new Date(sessionStart).getTime() + 30 * DAY).toISOString();
        const balance = add('player_plan_balances', `balance-${n}`, {
          playerId: player,
          sourcePlanId: plan.id,
          kind: 'time',
          deviceType: 'PC',
          deviceSubType: 'HIGH_END_PCS',
          remainingMinutes: minutes - used,
          status: live ? 'active' : day < 30 ? 'expired' : 'exhausted',
          expiryDate: expiry,
          ...stamp(sessionStart),
        });
        const session = add('usage_sessions', `session-${n}`, {
          deviceId: devices[visit % 11],
          balanceId: balance,
          startTime: sessionStart,
          endTime: live ? null : time,
          durationMinutes: live ? null : minutes,
          timeCreditsConsumed: used,
          walletMinutesAtStart: minutes,
          sourcePlanIdAtStart: plan.id,
          ...stamp(sessionStart),
        });
        add('player_plan_ledger', `purchase-ledger-${n}`, {
          balanceId: balance,
          playerId: player,
          deltaMinutes: minutes,
          reason: 'purchase',
          transactionId: transaction,
          balanceAfter: minutes,
          expiryAfter: expiry,
          createdAt: sessionStart,
        });
        if (live) tables.devices[visit % 11].status = 'in_use';
        if (!live)
          add('player_plan_ledger', `usage-ledger-${n}`, {
            balanceId: balance,
            playerId: player,
            deltaMinutes: -minutes,
            reason: 'session_usage',
            sessionId: session,
            balanceAfter: 0,
            expiryAfter: expiry,
            createdAt: time,
          });
      }
      if (paid) {
        const settlement = add('credit_settlements', `settlement-${n}`, {
          playerId: player,
          shiftId: shift,
          settledBy: staff,
          amount: paid,
          paymentMethod: 'online',
          settledAt: time,
          notes: 'DEMO partial collection',
          ...stamp(time),
        });
        add('credit_settlement_items', `settlement-item-${n}`, {
          settlementId: settlement,
          transactionId: transaction,
          amountApplied: paid,
          createdAt: time,
        });
      }
    }
    const expenseStatus = day % 13 === 0 ? 'pending' : day % 17 === 0 ? 'rejected' : 'approved';
    const expenseAmount = 150 + (day % 7) * 35;
    add('expenses', `expense-${day}`, {
      categoryId: categories[day % 3],
      vendorId: vendor,
      amount: expenseAmount,
      paymentMethod: 'online',
      approvalStatus: expenseStatus,
      expenseDate: at(day, 15),
      shiftId: shift,
      approvedBy: expenseStatus === 'approved' ? staff : null,
      approvedAt: expenseStatus === 'approved' ? at(day, 16) : null,
      description: 'DEMO operating expense',
      ...stamp(at(day, 15)),
    });
    const variance = day % 10 === 0 ? -20 : day % 9 === 0 ? 10 : 0;
    add('cash_registers', `register-${day}`, {
      shiftId: shift,
      openedBy: staff,
      closedBy: staff,
      openingBalance: 500,
      expectedClosing: day === 59 ? null : 500 + cash,
      closingBalance: day === 59 ? null : 500 + cash + variance,
      variance: day === 59 ? null : variance,
      status: day === 59 ? 'open' : day > 55 ? 'closed' : 'reconciled',
      notes: 'DEMO daily reconciliation',
      ...stamp(at(day, 20)),
    });
    if (day % 5 === 0) {
      const status = day === 55 ? 'pending' : day === 50 ? 'rejected' : 'approved';
      const amount = 100;
      const deposit = add('cash_deposits', `deposit-${day}`, {
        cashRegisterId: registerId,
        shiftId: shift,
        initiatedBy: staff,
        approvedBy: status === 'approved' ? staff : null,
        amount,
        denominations: { 100: 1 },
        depositType: day % 10 === 0 ? 'bank' : 'home',
        status,
        approvedAt: status === 'approved' ? at(day, 19) : null,
        notes: 'DEMO deposit',
        ...stamp(at(day, 18)),
      });
      if (status === 'approved') {
        entry(`deposit-entry-${day}`, 'cash_out', amount, deposit, 'cash_deposit', at(day, 19));
        tables.cash_registers.at(-1).expectedClosing -= amount;
        tables.cash_registers.at(-1).closingBalance -= amount;
      }
    }
  }
  // Stock receipts exactly cover sales, approved waste, and a small closing stock.
  for (let i = 0; i < products.length; i++) {
    const p = products[i];
    p.waste = 2 + i;
    const boxes = Math.ceil((p.sold + p.waste) / 12);
    const remaining = boxes * 12 - p.sold - p.waste;
    const receipt = add('stock_receipts', `receipt-${i}`, {
      locationId: location,
      vendorId: vendor,
      notes: 'DEMO opening stock',
      createdAt: at(0, 0),
      createdBy: staff,
    });
    add('stock_receipt_lines', `receipt-line-${i}`, {
      receiptId: receipt,
      productId: p.id,
      boxQuantity: boxes,
      piecesAdded: boxes * 12,
    });
    add('stock_movements', `receipt-movement-${i}`, {
      locationId: location,
      productId: p.id,
      delta: boxes * 12,
      movementType: 'receipt',
      referenceId: receipt,
      referenceType: 'stock_receipt',
      createdAt: at(0, 0),
    });
    const waste = add('stock_waste_events', `waste-${i}`, {
      locationId: location,
      status: 'approved',
      notes: 'DEMO spoilage',
      approvedBy: staff,
      approvedAt: at(58, 17),
      ...stamp(at(58, 16)),
    });
    add('stock_waste_lines', `waste-line-${i}`, {
      wasteEventId: waste,
      productId: p.id,
      quantityPieces: p.waste,
      reasonCode: 'expired',
    });
    add('stock_movements', `waste-movement-${i}`, {
      locationId: location,
      productId: p.id,
      delta: -p.waste,
      movementType: 'waste',
      referenceId: waste,
      referenceType: 'stock_waste',
      createdAt: at(58, 17),
    });
    tables.location_stock ??= [];
    tables.location_stock.push({
      locationId: location,
      productId: p.id,
      quantityPieces: remaining,
    });
    tables.products[i].stockQuantity = remaining;
    add('inventory_reorder_rules', `reorder-${i}`, {
      locationId: location,
      productId: p.id,
      minimumPieces: 12,
      targetPieces: 48,
      preferredVendorId: vendor,
      isActive: true,
      ...stamp(at(0, 0)),
    });
  }
  return {
    version: PREFIX,
    startDate: new Date(firstDay).toISOString().slice(0, 10),
    endDate: end.toISOString().slice(0, 10),
    tables,
  };
}

if (process.argv[1] && import.meta.url === new URL(`file://${process.argv[1]}`).href) {
  process.stdout.write(JSON.stringify(buildDemo(new Date('2026-10-02T23:59:59Z'))));
}
