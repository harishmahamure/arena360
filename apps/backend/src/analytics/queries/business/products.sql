SELECT CAST(p.id AS VARCHAR),p.name,sum(l.quantity)::BIGINT,sum(l.quantity*l.unit_price),COUNT(DISTINCT t.id)
 FROM report_transaction_lines l INNER JOIN report_transactions t ON t.id=l.transaction_id INNER JOIN products p ON p.id=l.product_id
 WHERE TRUE AND t.payment_status IN ('completed','credit') AND t.occurred_at>=$1 AND t.occurred_at<$2
 GROUP BY p.id,p.name ORDER BY 4 DESC LIMIT 100
