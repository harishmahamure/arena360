SELECT
    COALESCE(SUM(
        CASE WHEN t.transaction_type::VARCHAR = 'plan_purchase' THEN
            CASE
                WHEN cs.payment_method = 'cash' THEN csi.amount_applied
                WHEN cs.payment_method = 'online' THEN 0
                WHEN cs.payment_method = 'split_payment' AND cs.amount > 0 THEN
                    csi.amount_applied * (COALESCE(cs.cash_amount, 0) / nullif(cs.amount, 0))
                ELSE 0
            END
        ELSE 0 END
    ), 0) AS plan_cash,
    COALESCE(SUM(
        CASE WHEN t.transaction_type::VARCHAR = 'plan_purchase' THEN
            CASE
                WHEN cs.payment_method = 'online' THEN csi.amount_applied
                WHEN cs.payment_method = 'cash' THEN 0
                WHEN cs.payment_method = 'split_payment' AND cs.amount > 0 THEN
                    csi.amount_applied * (COALESCE(cs.online_amount, 0) / nullif(cs.amount, 0))
                ELSE 0
            END
        ELSE 0 END
    ), 0) AS plan_online,
    COALESCE(SUM(
        CASE WHEN t.transaction_type::VARCHAR = 'product_purchase' THEN
            CASE
                WHEN cs.payment_method = 'cash' THEN csi.amount_applied
                WHEN cs.payment_method = 'online' THEN 0
                WHEN cs.payment_method = 'split_payment' AND cs.amount > 0 THEN
                    csi.amount_applied * (COALESCE(cs.cash_amount, 0) / nullif(cs.amount, 0))
                ELSE 0
            END
        ELSE 0 END
    ), 0) AS product_cash,
    COALESCE(SUM(
        CASE WHEN t.transaction_type::VARCHAR = 'product_purchase' THEN
            CASE
                WHEN cs.payment_method = 'online' THEN csi.amount_applied
                WHEN cs.payment_method = 'cash' THEN 0
                WHEN cs.payment_method = 'split_payment' AND cs.amount > 0 THEN
                    csi.amount_applied * (COALESCE(cs.online_amount, 0) / nullif(cs.amount, 0))
                ELSE 0
            END
        ELSE 0 END
    ), 0) AS product_online
FROM report_credit_settlements cs
INNER JOIN report_credit_settlement_items csi ON csi.settlement_id = cs.id
INNER JOIN report_transactions t ON t.id = csi.transaction_id
WHERE cs.settled_at BETWEEN $1 AND $2
  AND TRUE
