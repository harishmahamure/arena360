WITH sales AS (
 SELECT * FROM transactions WHERE "deletedAt" IS NULL AND "transactionDate">=$1 AND "transactionDate"<$2
), booked AS (
 SELECT * FROM sales WHERE "paymentStatus"::text IN ('completed','credit')
), spend AS (
 SELECT e.*,c.name category FROM expenses e JOIN expense_categories c ON c.id=e."categoryId"
 WHERE e."deletedAt" IS NULL AND e."expenseDate">=$1 AND e."expenseDate"<$2
), dates AS (
 SELECT generate_series($1 AT TIME ZONE 'UTC',($2 AT TIME ZONE 'UTC')-interval '1 day',interval '1 day')::date AS report_day
)
SELECT jsonb_build_object(
 'generatedAt',now(),
 'sales',coalesce((SELECT sum(amount) FROM booked),0)::text,
 'saleCount',(SELECT count(*) FROM booked),
 'approvedExpenses',coalesce((SELECT sum(amount) FROM spend WHERE "approvalStatus"='approved'),0)::text,
 'pendingExpenses',coalesce((SELECT sum(amount) FROM spend WHERE "approvalStatus"='pending'),0)::text,
 'pendingExpenseCount',(SELECT count(*) FROM spend WHERE "approvalStatus"='pending'),
 'refundedSales',coalesce((SELECT sum(amount) FROM sales WHERE "paymentStatus"::text='refunded'),0)::text,
 'pendingSales',coalesce((SELECT sum(amount) FROM sales WHERE "paymentStatus"::text='pending'),0)::text,
 'currentOutstanding',coalesce((SELECT sum(greatest(amount-"paidAmount",0)) FROM transactions
   WHERE "deletedAt" IS NULL AND "paymentMethod"::text='credit' AND "paymentStatus"::text IN ('credit','completed')),0)::text,
 'creditCollections',coalesce((SELECT sum(amount) FROM credit_settlements WHERE "deletedAt" IS NULL AND "settledAt">=$1 AND "settledAt"<$2),0)::text,
 'salesByType',(SELECT coalesce(jsonb_agg(x ORDER BY x->>'label'),'[]'::jsonb) FROM (
   SELECT jsonb_build_object('label',"transactionType"::text,'amount',sum(amount)::text,'count',count(*)) x FROM booked GROUP BY "transactionType") q),
 'salesByPayment',(SELECT coalesce(jsonb_agg(x ORDER BY x->>'label'),'[]'::jsonb) FROM (
   SELECT jsonb_build_object('label',"paymentMethod"::text,'amount',sum(amount)::text,'count',count(*)) x FROM booked GROUP BY "paymentMethod") q),
 'expensesByCategory',(SELECT coalesce(jsonb_agg(x ORDER BY x->>'label'),'[]'::jsonb) FROM (
   SELECT jsonb_build_object('label',category,'amount',sum(amount)::text,'count',count(*)) x FROM spend WHERE "approvalStatus"='approved' GROUP BY "categoryId",category) q),
 'daily',(SELECT jsonb_agg(jsonb_build_object('date',d.report_day,'sales',coalesce(s.amount,0)::text,'expenses',coalesce(e.amount,0)::text) ORDER BY d.report_day)
   FROM dates d LEFT JOIN (SELECT ("transactionDate" AT TIME ZONE 'UTC')::date AS report_day,sum(amount) amount FROM booked GROUP BY 1) s ON s.report_day=d.report_day
   LEFT JOIN (SELECT ("expenseDate" AT TIME ZONE 'UTC')::date AS report_day,sum(amount) amount FROM spend WHERE "approvalStatus"='approved' GROUP BY 1) e ON e.report_day=d.report_day)
)
