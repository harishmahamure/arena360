SELECT
    COALESCE(SUM(cs.amount), 0) AS settlement_total,
    COALESCE(SUM(
        CASE
            WHEN cs.payment_method = 'cash' THEN cs.amount
            WHEN cs.payment_method = 'split_payment' THEN COALESCE(cs.cash_amount, 0)
            ELSE 0
        END
    ), 0) AS settlement_cash,
    COALESCE(SUM(
        CASE
            WHEN cs.payment_method = 'online' THEN cs.amount
            WHEN cs.payment_method = 'split_payment' THEN COALESCE(cs.online_amount, 0)
            ELSE 0
        END
    ), 0) AS settlement_online
FROM report_credit_settlements cs
WHERE cs.settled_at BETWEEN $1 AND $2
  AND TRUE
