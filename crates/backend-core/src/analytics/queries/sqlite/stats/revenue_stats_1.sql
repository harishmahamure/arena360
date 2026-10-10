SELECT
    report_money(COALESCE(SUM(CASE WHEN transaction_type = 'plan_purchase' THEN amount ELSE 0 END), 0)) AS plan,
    report_money(COALESCE(SUM(CASE WHEN transaction_type = 'product_purchase' THEN amount ELSE 0 END), 0)) AS merchandise,
    report_money(COALESCE(SUM(
        CASE
            WHEN payment_method = 'cash' THEN amount
            WHEN payment_method = 'split_payment' THEN COALESCE(cash_amount, 0)
            ELSE 0
        END
    ), 0)) AS cash_revenue,
    report_money(COALESCE(SUM(
        CASE
            WHEN payment_method = 'online' THEN amount
            WHEN payment_method = 'split_payment' THEN COALESCE(online_amount, 0)
            ELSE 0
        END
    ), 0)) AS online_revenue,
    report_money(COALESCE(SUM(
        CASE WHEN payment_method = 'credit' THEN amount ELSE 0 END
    ), 0)) AS credit_revenue,
    COUNT(*) FILTER (WHERE transaction_type = 'plan_purchase') AS plan_transaction_count,
    COUNT(*) FILTER (WHERE transaction_type = 'product_purchase') AS product_transaction_count,
    report_money(COALESCE(SUM(
        CASE
            WHEN transaction_type = 'plan_purchase' AND payment_method = 'cash' THEN amount
            WHEN transaction_type = 'plan_purchase' AND payment_method = 'split_payment' THEN COALESCE(cash_amount, 0)
            ELSE 0
        END
    ), 0)) AS plan_cash_revenue,
    report_money(COALESCE(SUM(
        CASE
            WHEN transaction_type = 'plan_purchase' AND payment_method = 'online' THEN amount
            WHEN transaction_type = 'plan_purchase' AND payment_method = 'split_payment' THEN COALESCE(online_amount, 0)
            ELSE 0
        END
    ), 0)) AS plan_online_revenue,
    report_money(COALESCE(SUM(
        CASE
            WHEN transaction_type = 'plan_purchase' AND payment_method = 'credit' THEN amount
            ELSE 0
        END
    ), 0)) AS plan_credit_revenue,
    report_money(COALESCE(SUM(
        CASE
            WHEN transaction_type = 'product_purchase' AND payment_method = 'cash' THEN amount
            WHEN transaction_type = 'product_purchase' AND payment_method = 'split_payment' THEN COALESCE(cash_amount, 0)
            ELSE 0
        END
    ), 0)) AS product_cash_revenue,
    report_money(COALESCE(SUM(
        CASE
            WHEN transaction_type = 'product_purchase' AND payment_method = 'online' THEN amount
            WHEN transaction_type = 'product_purchase' AND payment_method = 'split_payment' THEN COALESCE(online_amount, 0)
            ELSE 0
        END
    ), 0)) AS product_online_revenue,
    report_money(COALESCE(SUM(
        CASE
            WHEN transaction_type = 'product_purchase' AND payment_method = 'credit' THEN amount
            ELSE 0
        END
    ), 0)) AS product_credit_revenue,
    COUNT(*) FILTER (WHERE transaction_type = 'plan_purchase'
          AND (
            payment_method = 'cash'
            OR (payment_method = 'split_payment' AND COALESCE(cash_amount, 0) > 0)
          )) AS plan_cash_count,
    COUNT(*) FILTER (WHERE transaction_type = 'plan_purchase'
          AND (
            payment_method = 'online'
            OR (payment_method = 'split_payment' AND COALESCE(online_amount, 0) > 0)
          )) AS plan_online_count,
    COUNT(*) FILTER (WHERE transaction_type = 'plan_purchase' AND payment_method = 'credit') AS plan_credit_count,
    COUNT(*) FILTER (WHERE transaction_type = 'product_purchase'
          AND (
            payment_method = 'cash'
            OR (payment_method = 'split_payment' AND COALESCE(cash_amount, 0) > 0)
          )) AS product_cash_count,
    COUNT(*) FILTER (WHERE transaction_type = 'product_purchase'
          AND (
            payment_method = 'online'
            OR (payment_method = 'split_payment' AND COALESCE(online_amount, 0) > 0)
          )) AS product_online_count,
    COUNT(*) FILTER (WHERE transaction_type = 'product_purchase' AND payment_method = 'credit') AS product_credit_count
FROM report_transactions
WHERE created_at BETWEEN $1 AND $2
  AND TRUE
  AND payment_status IN ('completed', 'credit')
