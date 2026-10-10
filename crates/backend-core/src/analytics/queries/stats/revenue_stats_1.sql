SELECT
    COALESCE(SUM(CASE WHEN transaction_type::VARCHAR = 'plan_purchase' THEN amount ELSE 0 END), 0) AS plan,
    COALESCE(SUM(CASE WHEN transaction_type::VARCHAR = 'product_purchase' THEN amount ELSE 0 END), 0) AS merchandise,
    COALESCE(SUM(
        CASE
            WHEN payment_method::VARCHAR = 'cash' THEN amount
            WHEN payment_method::VARCHAR = 'split_payment' THEN COALESCE(cash_amount, 0)
            ELSE 0
        END
    ), 0) AS cash_revenue,
    COALESCE(SUM(
        CASE
            WHEN payment_method::VARCHAR = 'online' THEN amount
            WHEN payment_method::VARCHAR = 'split_payment' THEN COALESCE(online_amount, 0)
            ELSE 0
        END
    ), 0) AS online_revenue,
    COALESCE(SUM(
        CASE WHEN payment_method::VARCHAR = 'credit' THEN amount ELSE 0 END
    ), 0) AS credit_revenue,
    COUNT(*) FILTER (WHERE transaction_type::VARCHAR = 'plan_purchase') AS plan_transaction_count,
    COUNT(*) FILTER (WHERE transaction_type::VARCHAR = 'product_purchase') AS product_transaction_count,
    COALESCE(SUM(
        CASE
            WHEN transaction_type::VARCHAR = 'plan_purchase' AND payment_method::VARCHAR = 'cash' THEN amount
            WHEN transaction_type::VARCHAR = 'plan_purchase' AND payment_method::VARCHAR = 'split_payment' THEN COALESCE(cash_amount, 0)
            ELSE 0
        END
    ), 0) AS plan_cash_revenue,
    COALESCE(SUM(
        CASE
            WHEN transaction_type::VARCHAR = 'plan_purchase' AND payment_method::VARCHAR = 'online' THEN amount
            WHEN transaction_type::VARCHAR = 'plan_purchase' AND payment_method::VARCHAR = 'split_payment' THEN COALESCE(online_amount, 0)
            ELSE 0
        END
    ), 0) AS plan_online_revenue,
    COALESCE(SUM(
        CASE
            WHEN transaction_type::VARCHAR = 'plan_purchase' AND payment_method::VARCHAR = 'credit' THEN amount
            ELSE 0
        END
    ), 0) AS plan_credit_revenue,
    COALESCE(SUM(
        CASE
            WHEN transaction_type::VARCHAR = 'product_purchase' AND payment_method::VARCHAR = 'cash' THEN amount
            WHEN transaction_type::VARCHAR = 'product_purchase' AND payment_method::VARCHAR = 'split_payment' THEN COALESCE(cash_amount, 0)
            ELSE 0
        END
    ), 0) AS product_cash_revenue,
    COALESCE(SUM(
        CASE
            WHEN transaction_type::VARCHAR = 'product_purchase' AND payment_method::VARCHAR = 'online' THEN amount
            WHEN transaction_type::VARCHAR = 'product_purchase' AND payment_method::VARCHAR = 'split_payment' THEN COALESCE(online_amount, 0)
            ELSE 0
        END
    ), 0) AS product_online_revenue,
    COALESCE(SUM(
        CASE
            WHEN transaction_type::VARCHAR = 'product_purchase' AND payment_method::VARCHAR = 'credit' THEN amount
            ELSE 0
        END
    ), 0) AS product_credit_revenue,
    COUNT(*) FILTER (WHERE transaction_type::VARCHAR = 'plan_purchase'
          AND (
            payment_method::VARCHAR = 'cash'
            OR (payment_method::VARCHAR = 'split_payment' AND COALESCE(cash_amount, 0) > 0)
          )) AS plan_cash_count,
    COUNT(*) FILTER (WHERE transaction_type::VARCHAR = 'plan_purchase'
          AND (
            payment_method::VARCHAR = 'online'
            OR (payment_method::VARCHAR = 'split_payment' AND COALESCE(online_amount, 0) > 0)
          )) AS plan_online_count,
    COUNT(*) FILTER (WHERE transaction_type::VARCHAR = 'plan_purchase' AND payment_method::VARCHAR = 'credit') AS plan_credit_count,
    COUNT(*) FILTER (WHERE transaction_type::VARCHAR = 'product_purchase'
          AND (
            payment_method::VARCHAR = 'cash'
            OR (payment_method::VARCHAR = 'split_payment' AND COALESCE(cash_amount, 0) > 0)
          )) AS product_cash_count,
    COUNT(*) FILTER (WHERE transaction_type::VARCHAR = 'product_purchase'
          AND (
            payment_method::VARCHAR = 'online'
            OR (payment_method::VARCHAR = 'split_payment' AND COALESCE(online_amount, 0) > 0)
          )) AS product_online_count,
    COUNT(*) FILTER (WHERE transaction_type::VARCHAR = 'product_purchase' AND payment_method::VARCHAR = 'credit') AS product_credit_count
FROM report_transactions
WHERE created_at BETWEEN $1 AND $2
  AND TRUE
  AND payment_status::VARCHAR IN ('completed', 'credit')
