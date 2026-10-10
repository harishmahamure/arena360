use super::report_reader::ReportReader;
use crate::{error::AppError, models::*};
use chrono::{DateTime, Utc};
use uuid::Uuid;

impl ReportReader {
    pub async fn get_summary_by_category(&self) -> Result<Vec<ExpenseSummaryDto>, AppError> {
        let summaries = self
            .query::<ExpenseSummaryDto>(
                r#"
            SELECT
                ec.name as category_name,
                report_money(ec.budget_amount) as budget_amount,
                ec.budget_period as budget_period,
                report_money(COALESCE(SUM(e.amount), 0)) as total_spent,
                CASE
                    WHEN ec.budget_amount IS NOT NULL
                    THEN report_money(ec.budget_amount - COALESCE(SUM(e.amount), 0))
                    ELSE NULL
                END as remaining_budget,
                COUNT(e.id) as expense_count
            FROM expense_categories ec
            LEFT JOIN report_expenses e ON e.category_id = ec.id
                AND TRUE
                AND e.approval_status = 'approved'
            WHERE ec.is_active = true
            GROUP BY ec.id, ec.name, ec.budget_amount, ec.budget_period
            ORDER BY total_spent DESC
            "#,
            )
            .fetch_all()
            .await?;

        Ok(summaries)
    }
    pub async fn receipt_summary(
        &self,
        location_id: Option<Uuid>,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<crate::models::ReceiptSummaryRow>, AppError> {
        let mut sql = String::from(
            r#"
            SELECT rl.product_id as product_id,
                   p.name as product_name,
                   sr.vendor_id as vendor_id,
                   v.name as vendor_name,
                   SUM(rl.box_quantity) as total_boxes,
                   SUM(rl.pieces_added) as total_pieces,
                   report_money(SUM(
                     rl.box_quantity *
                     COALESCE(p.purchase_price_per_box, p.purchase_price, 0)
                   )) as estimated_cost
            FROM report_stock_receipt_lines rl
            INNER JOIN report_stock_receipts sr ON sr.id = rl.receipt_id
            INNER JOIN products p ON p.id = rl.product_id
            LEFT JOIN vendors v ON v.id = sr.vendor_id
            WHERE 1=1
            "#,
        );
        let mut parameter_count = 0;
        if location_id.is_some() {
            parameter_count += 1;
            sql.push_str(&format!(
                " AND sr.location_id = ${parameters_len}",
                parameters_len = parameter_count
            ));
        }
        if from.is_some() {
            parameter_count += 1;
            sql.push_str(&format!(
                " AND sr.received_at >= ${parameters_len}",
                parameters_len = parameter_count
            ));
        }
        if to.is_some() {
            parameter_count += 1;
            sql.push_str(&format!(
                " AND sr.received_at <= ${parameters_len}",
                parameters_len = parameter_count
            ));
        }
        sql.push_str(
            r#"
            GROUP BY rl.product_id, p.name, sr.vendor_id, v.name
            ORDER BY p.name ASC, v.name ASC NULLS LAST
            "#,
        );
        let mut query = self.query(sql);
        if let Some(value) = location_id {
            query = query.bind(value);
        }
        if let Some(value) = from {
            query = query.bind(value);
        }
        if let Some(value) = to {
            query = query.bind(value);
        }
        query.fetch_all().await
    }
    pub async fn waste_summary(
        &self,
        location_id: Option<Uuid>,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<WasteSummaryRow>, AppError> {
        let mut sql = String::from(
            r#"
            SELECT wl.reason_code as reason_code,
                   wl.product_id as product_id,
                   p.name as product_name,
                   we.location_id as location_id,
                   il.name as location_name,
                   SUM(wl.quantity_pieces) as total_pieces,
                   report_money(SUM(
                     wl.quantity_pieces * 1.0 *
                     COALESCE(p.purchase_price_per_box, p.purchase_price, 0) /
                     max(p.units_per_purchase_unit, 1)
                   )) as estimated_cost
            FROM report_stock_waste_lines wl
            INNER JOIN report_stock_waste_events we ON we.id = wl.waste_event_id
            INNER JOIN products p ON p.id = wl.product_id
            INNER JOIN report_inventory_locations il ON il.id = we.location_id
            WHERE we.status = 'approved'
            "#,
        );
        let mut parameter_count = 0;
        if location_id.is_some() {
            parameter_count += 1;
            sql.push_str(&format!(
                " AND we.location_id = ${parameters_len}",
                parameters_len = parameter_count
            ));
        }
        if from.is_some() {
            parameter_count += 1;
            sql.push_str(&format!(
                " AND we.approved_at >= ${parameters_len}",
                parameters_len = parameter_count
            ));
        }
        if to.is_some() {
            parameter_count += 1;
            sql.push_str(&format!(
                " AND we.approved_at <= ${parameters_len}",
                parameters_len = parameter_count
            ));
        }
        sql.push_str(
            r#" GROUP BY wl.reason_code, wl.product_id, p.name, we.location_id, il.name
                ORDER BY total_pieces DESC"#,
        );
        let mut query = self.query(sql);
        if let Some(value) = location_id {
            query = query.bind(value);
        }
        if let Some(value) = from {
            query = query.bind(value);
        }
        if let Some(value) = to {
            query = query.bind(value);
        }
        query.fetch_all().await
    }
    pub async fn get_portfolio_summary(&self) -> Result<CreditPortfolioSummary, AppError> {
        #[derive(serde::Deserialize)]
        struct AggregateRow {
            total_credit_limit: f64,
            total_outstanding: f64,
            total_available: f64,
            credit_enabled_player_count: i64,
            players_with_outstanding_count: i64,
        }

        let agg = self.query::< AggregateRow>(
            r#"
            WITH outstanding_by_player AS (
                SELECT
                    t.player_id AS player_id,
                    SUM(t.amount - t.paid_amount) AS outstanding
                FROM report_transactions t
                WHERE t.payment_method = 'credit'
                  AND t.payment_status = 'credit'
                  AND TRUE
                GROUP BY t.player_id
            ),
            credit_players AS (
                SELECT
                    u.id,
                    u.credit_limit AS credit_limit,
                    COALESCE(o.outstanding, 0) AS outstanding
                FROM report_users u
                LEFT JOIN outstanding_by_player o ON o.player_id = u.id
                WHERE TRUE
                  AND u.role = 'player'
                  AND u.is_active = true
                  AND u.credit_limit > 0
            )
            SELECT
                report_money((SELECT COALESCE(SUM(credit_limit), 0) FROM credit_players)) AS total_credit_limit,
                report_money((SELECT COALESCE(SUM(outstanding), 0) FROM outstanding_by_player)) AS total_outstanding,
                report_money((SELECT COALESCE(SUM(max(credit_limit - outstanding, 0)), 0) FROM credit_players)) AS total_available,
                (SELECT COUNT(*) FROM credit_players) AS credit_enabled_player_count,
                (SELECT COUNT(*) FROM outstanding_by_player WHERE outstanding > 0) AS players_with_outstanding_count
            "#,
        )
        .fetch_one()
        .await?;

        let last_settlement = self
            .query::<CreditLastSettlement>(
                r#"
            SELECT
                cs.id,
                cs.player_id AS player_id,
                player.username AS player_username,
                report_money(cs.amount) AS amount,
                cs.settled_at AS settled_at
            FROM report_credit_settlements cs
            INNER JOIN report_users player ON player.id = cs.player_id
            WHERE TRUE
            ORDER BY cs.settled_at DESC
            LIMIT 1
            "#,
            )
            .fetch_optional()
            .await?;

        Ok(CreditPortfolioSummary {
            total_credit_limit: agg.total_credit_limit,
            total_outstanding: agg.total_outstanding,
            total_available: agg.total_available,
            utilization_percent: compute_utilization_percent(
                agg.total_credit_limit,
                agg.total_outstanding,
            ),
            credit_enabled_player_count: agg.credit_enabled_player_count,
            players_with_outstanding_count: agg.players_with_outstanding_count,
            last_settlement,
        })
    }

    pub async fn overview(&self) -> Result<InventoryOverviewDto, AppError> {
        let (pieces,value):(i64,f64)=self.query(r#"SELECT COALESCE(SUM(ls.quantity_pieces),0),report_money(COALESCE(SUM(ls.quantity_pieces*COALESCE(p.purchase_price_per_box*1.0/NULLIF(p.units_per_purchase_unit,0),p.purchase_price,0)),0)) FROM report_location_stock ls JOIN products p ON p.id=ls.product_id"#).fetch_one().await?;
        let (low,): (i64,)=self.query(r#"SELECT COUNT(*) FROM report_reorder_rules r LEFT JOIN report_location_stock ls ON ls.location_id=r.location_id AND ls.product_id=r.product_id WHERE r.is_active=true AND COALESCE(ls.quantity_pieces,0)>0 AND COALESCE(ls.quantity_pieces,0)<=r.minimum_pieces"#).fetch_one().await?;
        let (out,): (i64,)=self.query(r#"SELECT COUNT(*) FROM report_reorder_rules r LEFT JOIN report_location_stock ls ON ls.location_id=r.location_id AND ls.product_id=r.product_id WHERE r.is_active=true AND COALESCE(ls.quantity_pieces,0)=0"#).fetch_one().await?;
        let (po,): (i64,)=self.query("SELECT COUNT(*) FROM report_purchase_orders WHERE status IN ('submitted','approved','ordered','partially_received')").fetch_one().await?;
        let (transfers,): (i64,) = self.query(
            "SELECT COUNT(*) FROM report_stock_transfer_requests WHERE status IN ('pending','approved')",
        )
        .fetch_one()
        .await?;
        let (waste,): (i64,) = self
            .query("SELECT COUNT(*) FROM report_stock_waste_events WHERE status='pending'")
            .fetch_one()
            .await?;
        let recent = self.query(r#"SELECT m.id,m.location_id,l.name,m.product_id,p.name,m.delta,m.movement_type,m.reference_id,m.reference_type,m.created_by,m.created_at
            FROM report_stock_movements m JOIN report_inventory_locations l ON l.id=m.location_id
            JOIN products p ON p.id=m.product_id ORDER BY m.created_at DESC,m.id ASC LIMIT 8"#).fetch_all().await?;
        Ok(InventoryOverviewDto {
            total_pieces: pieces,
            estimated_stock_value: value,
            low_stock_products: low,
            out_of_stock_products: out,
            open_purchase_orders: po,
            pending_transfers: transfers,
            pending_waste_events: waste,
            recent_movements: recent,
        })
    }
}

impl ReportReader {
    /// Booked sales and collections are separate. Decimal strings preserve ledger precision.
    pub async fn finance_report(
        &self,
        start: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<serde_json::Value, AppError> {
        let sales: (String, i64, String, String) = self.query(r#"
            SELECT report_money_text(COALESCE(COALESCE(SUM(amount) FILTER (WHERE payment_status IN ('completed','credit')),0),0),2),
                   COUNT(*) FILTER (WHERE payment_status IN ('completed','credit')),
                   report_money_text(COALESCE(COALESCE(SUM(amount) FILTER (WHERE payment_status='refunded'),0),0),2),
                   report_money_text(COALESCE(COALESCE(SUM(amount) FILTER (WHERE payment_status='pending'),0),0),2)
            FROM report_transactions WHERE TRUE AND occurred_at>=$1 AND occurred_at<$2
        "#).bind(start).bind(until).fetch_one().await?;
        let expenses: (String, String, i64) = self.query(r#"
            SELECT report_money_text(COALESCE(COALESCE(SUM(e.amount) FILTER (WHERE e.approval_status='approved'),0),0),4),
                   report_money_text(COALESCE(COALESCE(SUM(e.amount) FILTER (WHERE e.approval_status='pending'),0),0),4), COUNT(*) FILTER (WHERE e.approval_status='pending')
            FROM report_expenses e JOIN expense_categories c ON c.id=e.category_id
            WHERE TRUE AND e.expense_date>=$1 AND e.expense_date<$2
        "#).bind(start).bind(until).fetch_one().await?;
        let (outstanding,): (String,) = self.query(r#"
            SELECT report_money_text(COALESCE(sum(max(amount-paid_amount,0)),0),4) FROM report_transactions
            WHERE TRUE AND payment_method='credit' AND payment_status IN ('credit','completed')
        "#).fetch_one().await?;
        let (collections,): (String,) = self
            .query(
                r#"
            SELECT report_money_text(COALESCE(sum(amount),0),4) FROM report_credit_settlements
            WHERE TRUE AND settled_at>=$1 AND settled_at<$2
        "#,
            )
            .bind(start)
            .bind(until)
            .fetch_one()
            .await?;
        let mut groups = Vec::new();
        for column in ["transaction_type", "payment_method"] {
            let rows: Vec<(String, String, i64)> = self.query(format!(r#"
                SELECT "{column}",report_money_text(COALESCE(sum(amount),0),2),count(*) FROM report_transactions
                WHERE TRUE AND occurred_at>=$1 AND occurred_at<$2
                  AND payment_status IN ('completed','credit') GROUP BY "{column}" ORDER BY "{column}"
            "#)).bind(start).bind(until).fetch_all().await?;
            groups.push(report_groups(rows));
        }
        let categories: Vec<(String, String, i64)> = self.query(r#"
            SELECT c.name,report_money_text(COALESCE(sum(e.amount),0),4),count(*) FROM report_expenses e JOIN expense_categories c ON c.id=e.category_id
            WHERE TRUE AND e.approval_status='approved' AND e.expense_date>=$1 AND e.expense_date<$2
            GROUP BY e.category_id,c.name ORDER BY c.name
        "#).bind(start).bind(until).fetch_all().await?;
        let daily_sales: Vec<(String, String)> = self.query(r#"
            SELECT date(occurred_at),report_money_text(COALESCE(sum(amount),0),2) FROM report_transactions
            WHERE TRUE AND payment_status IN ('completed','credit') AND occurred_at>=$1 AND occurred_at<$2
            GROUP BY date(occurred_at)
        "#).bind(start).bind(until).fetch_all().await?;
        let daily_expenses: Vec<(String, String)> = self.query(r#"
            SELECT date(e.expense_date),report_money_text(COALESCE(sum(e.amount),0),4) FROM report_expenses e JOIN expense_categories c ON c.id=e.category_id
            WHERE TRUE AND e.approval_status='approved' AND e.expense_date>=$1 AND e.expense_date<$2
            GROUP BY date(e.expense_date)
        "#).bind(start).bind(until).fetch_all().await?;
        let sales_by_day: std::collections::BTreeMap<_, _> = daily_sales.into_iter().collect();
        let expenses_by_day: std::collections::BTreeMap<_, _> =
            daily_expenses.into_iter().collect();
        let mut day = start.date_naive();
        let mut daily = vec![];
        while day < until.date_naive() {
            let key = day.to_string();
            daily.push(serde_json::json!({"date":key, "sales":sales_by_day.get(&key).map(String::as_str).unwrap_or("0"),
                "expenses":expenses_by_day.get(&key).map(String::as_str).unwrap_or("0")}));
            day = day
                .succ_opt()
                .ok_or_else(|| AppError::BadRequest("Invalid report range".into()))?;
        }
        Ok(serde_json::json!({
            "generatedAt": Utc::now(), "sales":sales.0,"saleCount":sales.1,"refundedSales":sales.2,"pendingSales":sales.3,
            "approvedExpenses":expenses.0,"pendingExpenses":expenses.1,"pendingExpenseCount":expenses.2,
            "currentOutstanding":outstanding,"creditCollections":collections,
            "salesByType":groups[0],"salesByPayment":groups[1],"expensesByCategory":report_groups(categories),"daily":daily
        }))
    }
}
fn report_groups(rows: Vec<(String, String, i64)>) -> Vec<serde_json::Value> {
    rows.into_iter().map(|(label,amount,count)| serde_json::json!({"label":label,"amount":amount,"count":count})).collect()
}
