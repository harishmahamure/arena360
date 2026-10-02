use super::{query_as, ClickHouse};
use crate::{error::AppError, models::*};
use chrono::{DateTime, Utc};
use uuid::Uuid;

impl ClickHouse {
    pub async fn get_summary_by_category(&self) -> Result<Vec<ExpenseSummaryDto>, AppError> {
        let summaries = query_as::<ExpenseSummaryDto>(
            r#"
            SELECT
                ec.name as category_name,
                ec."budgetAmount"::Float64 as budget_amount,
                ec."budgetPeriod" as budget_period,
                COALESCE(SUM(e.amount)::Float64, 0) as total_spent,
                CASE
                    WHEN ec."budgetAmount" IS NOT NULL
                    THEN (ec."budgetAmount" - COALESCE(SUM(e.amount), 0))::Float64
                    ELSE NULL
                END as remaining_budget,
                COUNT(e.id) as expense_count
            FROM expense_categories ec
            LEFT JOIN expenses e ON e."categoryId" = ec.id
                AND e."deletedAt" IS NULL
                AND e."approvalStatus" = 'approved'
            WHERE ec."isActive" = true
            GROUP BY ec.id, ec.name, ec."budgetAmount", ec."budgetPeriod"
            ORDER BY total_spent DESC
            "#,
        )
        .fetch_all(self)
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
            SELECT rl."productId" as product_id,
                   p.name as product_name,
                   sr."vendorId" as vendor_id,
                   v.name as vendor_name,
                   SUM(rl."boxQuantity")::Int64 as total_boxes,
                   SUM(rl."piecesAdded")::Int64 as total_pieces,
                   SUM(
                     rl."boxQuantity"::Float64 *
                     COALESCE(p."purchasePricePerBox", p."purchasePrice", 0)::Float64
                   )::Float64 as estimated_cost
            FROM stock_receipt_lines rl
            INNER JOIN stock_receipts sr ON sr.id = rl."receiptId"
            INNER JOIN products p ON p.id = rl."productId"
            LEFT JOIN vendors v ON v.id = sr."vendorId"
            WHERE 1=1
            "#,
        );
        let mut parameters = Vec::new();
        if let Some(value) = location_id {
            parameters.push(super::client::Parameter::parameter(value));
            sql.push_str(&format!(
                " AND sr.\"locationId\" = ${parameters_len}",
                parameters_len = parameters.len()
            ));
        }
        if let Some(value) = from {
            parameters.push(super::client::Parameter::parameter(value));
            sql.push_str(&format!(
                " AND sr.\"createdAt\" >= ${parameters_len}",
                parameters_len = parameters.len()
            ));
        }
        if let Some(value) = to {
            parameters.push(super::client::Parameter::parameter(value));
            sql.push_str(&format!(
                " AND sr.\"createdAt\" <= ${parameters_len}",
                parameters_len = parameters.len()
            ));
        }
        sql.push_str(
            r#"
            GROUP BY rl."productId", p.name, sr."vendorId", v.name
            ORDER BY p.name ASC, v.name ASC NULLS LAST
            "#,
        );
        let mut query = query_as(sql);
        for parameter in parameters {
            query = query.bind_raw(parameter);
        }
        query.fetch_all(self).await
    }
    pub async fn waste_summary(
        &self,
        location_id: Option<Uuid>,
        from: Option<chrono::DateTime<chrono::Utc>>,
        to: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Vec<WasteSummaryRow>, AppError> {
        let mut sql = String::from(
            r#"
            SELECT wl."reasonCode"::String as reason_code,
                   wl."productId" as product_id,
                   p.name as product_name,
                   we."locationId" as location_id,
                   il.name as location_name,
                   SUM(wl."quantityPieces")::Int64 as total_pieces,
                   SUM(
                     wl."quantityPieces"::Float64 *
                     COALESCE(p."purchasePricePerBox", p."purchasePrice", 0)::Float64 /
                     GREATEST(p."unitsPerPurchaseUnit", 1)
                   )::Float64 as estimated_cost
            FROM stock_waste_lines wl
            INNER JOIN stock_waste_events we ON we.id = wl."wasteEventId"
            INNER JOIN products p ON p.id = wl."productId"
            INNER JOIN inventory_locations il ON il.id = we."locationId"
            WHERE we.status = 'approved'
            "#,
        );
        let mut parameters = Vec::new();
        if let Some(value) = location_id {
            parameters.push(super::client::Parameter::parameter(value));
            sql.push_str(&format!(
                " AND we.\"locationId\" = ${parameters_len}",
                parameters_len = parameters.len()
            ));
        }
        if let Some(value) = from {
            parameters.push(super::client::Parameter::parameter(value));
            sql.push_str(&format!(
                " AND we.\"approvedAt\" >= ${parameters_len}",
                parameters_len = parameters.len()
            ));
        }
        if let Some(value) = to {
            parameters.push(super::client::Parameter::parameter(value));
            sql.push_str(&format!(
                " AND we.\"approvedAt\" <= ${parameters_len}",
                parameters_len = parameters.len()
            ));
        }
        sql.push_str(
            r#" GROUP BY wl."reasonCode", wl."productId", p.name, we."locationId", il.name
                ORDER BY total_pieces DESC"#,
        );
        let mut query = query_as(sql);
        for parameter in parameters {
            query = query.bind_raw(parameter);
        }
        query.fetch_all(self).await
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

        let agg = query_as::< AggregateRow>(
            r#"
            WITH outstanding_by_player AS (
                SELECT
                    t."playerId" AS player_id,
                    SUM(t.amount - t."paidAmount")::Float64 AS outstanding
                FROM transactions t
                WHERE t."paymentMethod" = 'credit'
                  AND t."paymentStatus" = 'credit'
                  AND t."deletedAt" IS NULL
                GROUP BY t."playerId"
            ),
            credit_players AS (
                SELECT
                    u.id,
                    u."creditLimit"::Float64 AS credit_limit,
                    COALESCE(o.outstanding, 0)::Float64 AS outstanding
                FROM users u
                LEFT JOIN outstanding_by_player o ON o.player_id = u.id
                WHERE u."deletedAt" IS NULL
                  AND u.role = 'player'
                  AND u."isActive" = true
                  AND u."creditLimit" > 0
            )
            SELECT
                (SELECT COALESCE(SUM(credit_limit), 0)::Float64 FROM credit_players) AS total_credit_limit,
                (SELECT COALESCE(SUM(outstanding), 0)::Float64 FROM outstanding_by_player) AS total_outstanding,
                (SELECT COALESCE(SUM(GREATEST(credit_limit - outstanding, 0)), 0)::Float64 FROM credit_players) AS total_available,
                (SELECT COUNT(*)::Int64 FROM credit_players) AS credit_enabled_player_count,
                (SELECT COUNT(*)::Int64 FROM outstanding_by_player WHERE outstanding > 0) AS players_with_outstanding_count
            "#,
        )
        .fetch_one(self)
        .await?;

        let last_settlement = query_as::<CreditLastSettlement>(
            r#"
            SELECT
                cs.id,
                cs."playerId" AS player_id,
                player.username AS player_username,
                cs.amount::Float64 AS amount,
                cs."settledAt" AS settled_at
            FROM credit_settlements cs
            INNER JOIN users player ON player.id = cs."playerId"
            WHERE cs."deletedAt" IS NULL
            ORDER BY cs."settledAt" DESC
            LIMIT 1
            "#,
        )
        .fetch_optional(self)
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
        let (pieces,value):(i64,f64)=query_as(r#"SELECT COALESCE(SUM(ls."quantityPieces"),0)::Int64,COALESCE(SUM(ls."quantityPieces"*COALESCE(p."purchasePricePerBox"/NULLIF(p."unitsPerPurchaseUnit",0),p."purchasePrice",0)),0)::Float64 FROM location_stock ls JOIN products p ON p.id=ls."productId""#).fetch_one(self).await?;
        let (low,): (i64,)=query_as(r#"SELECT COUNT(*) FROM inventory_reorder_rules r LEFT JOIN location_stock ls ON ls."locationId"=r."locationId" AND ls."productId"=r."productId" WHERE r."isActive"=true AND COALESCE(ls."quantityPieces",0)>0 AND COALESCE(ls."quantityPieces",0)<=r."minimumPieces""#).fetch_one(self).await?;
        let (out,): (i64,)=query_as(r#"SELECT COUNT(*) FROM inventory_reorder_rules r LEFT JOIN location_stock ls ON ls."locationId"=r."locationId" AND ls."productId"=r."productId" WHERE r."isActive"=true AND COALESCE(ls."quantityPieces",0)=0"#).fetch_one(self).await?;
        let (po,): (i64,)=query_as("SELECT COUNT(*) FROM purchase_orders WHERE status IN ('submitted','approved','ordered','partially_received')").fetch_one(self).await?;
        let (transfers,): (i64,) = query_as(
            "SELECT COUNT(*) FROM stock_transfer_requests WHERE status IN ('pending','approved')",
        )
        .fetch_one(self)
        .await?;
        let (waste,): (i64,) =
            query_as("SELECT COUNT(*) FROM stock_waste_events WHERE status='pending'")
                .fetch_one(self)
                .await?;
        let recent = query_as(r#"SELECT m.id,m."locationId",l.name,m."productId",p.name,m.delta,m."movementType",m."referenceId",m."referenceType",m."createdBy",m."createdAt"
            FROM stock_movements m JOIN inventory_locations l ON l.id=m."locationId"
            JOIN products p ON p.id=m."productId" ORDER BY m."createdAt" DESC LIMIT 8"#).fetch_all(self).await?;
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

impl ClickHouse {
    /// Booked sales and collections are separate. Decimal strings preserve ledger precision.
    pub async fn finance_report(
        &self,
        start: DateTime<Utc>,
        until: DateTime<Utc>,
    ) -> Result<serde_json::Value, AppError> {
        let sales: (String, i64, String, String) = query_as(r#"
            SELECT toDecimalString(sumIf(amount, "paymentStatus" IN ('completed','credit')), 2),
                   countIf("paymentStatus" IN ('completed','credit')),
                   toDecimalString(sumIf(amount, "paymentStatus"='refunded'), 2),
                   toDecimalString(sumIf(amount, "paymentStatus"='pending'), 2)
            FROM transactions WHERE "deletedAt" IS NULL AND "transactionDate">=$1 AND "transactionDate"<$2
        "#).bind(start).bind(until).fetch_one(self).await?;
        let expenses: (String, String, i64) = query_as(r#"
            SELECT toDecimalString(sumIf(e.amount, e."approvalStatus"='approved'), 4),
                   toDecimalString(sumIf(e.amount, e."approvalStatus"='pending'), 4), countIf(e."approvalStatus"='pending')
            FROM expenses e JOIN expense_categories c ON c.id=e."categoryId"
            WHERE e."deletedAt" IS NULL AND e."expenseDate">=$1 AND e."expenseDate"<$2
        "#).bind(start).bind(until).fetch_one(self).await?;
        let (outstanding,): (String,) = query_as(r#"
            SELECT toDecimalString(sum(greatest(amount-"paidAmount",0)), 4) FROM transactions
            WHERE "deletedAt" IS NULL AND "paymentMethod"='credit' AND "paymentStatus" IN ('credit','completed')
        "#).fetch_one(self).await?;
        let (collections,): (String,) = query_as(
            r#"
            SELECT toDecimalString(sum(amount), 4) FROM credit_settlements
            WHERE "deletedAt" IS NULL AND "settledAt">=$1 AND "settledAt"<$2
        "#,
        )
        .bind(start)
        .bind(until)
        .fetch_one(self)
        .await?;
        let mut groups = Vec::new();
        for column in ["transactionType", "paymentMethod"] {
            let rows: Vec<(String, String, i64)> = query_as(format!(r#"
                SELECT "{column}",toDecimalString(sum(amount), 2),count() FROM transactions
                WHERE "deletedAt" IS NULL AND "transactionDate">=$1 AND "transactionDate"<$2
                  AND "paymentStatus" IN ('completed','credit') GROUP BY "{column}" ORDER BY "{column}"
            "#)).bind(start).bind(until).fetch_all(self).await?;
            groups.push(report_groups(rows));
        }
        let categories: Vec<(String, String, i64)> = query_as(r#"
            SELECT c.name,toDecimalString(sum(e.amount), 4),count() FROM expenses e JOIN expense_categories c ON c.id=e."categoryId"
            WHERE e."deletedAt" IS NULL AND e."approvalStatus"='approved' AND e."expenseDate">=$1 AND e."expenseDate"<$2
            GROUP BY e."categoryId",c.name ORDER BY c.name
        "#).bind(start).bind(until).fetch_all(self).await?;
        let daily_sales: Vec<(String, String)> = query_as(r#"
            SELECT toString(toDate("transactionDate", 'UTC')),toDecimalString(sum(amount), 2) FROM transactions
            WHERE "deletedAt" IS NULL AND "paymentStatus" IN ('completed','credit') AND "transactionDate">=$1 AND "transactionDate"<$2
            GROUP BY toDate("transactionDate", 'UTC')
        "#).bind(start).bind(until).fetch_all(self).await?;
        let daily_expenses: Vec<(String, String)> = query_as(r#"
            SELECT toString(toDate(e."expenseDate", 'UTC')),toDecimalString(sum(e.amount), 4) FROM expenses e JOIN expense_categories c ON c.id=e."categoryId"
            WHERE e."deletedAt" IS NULL AND e."approvalStatus"='approved' AND e."expenseDate">=$1 AND e."expenseDate"<$2
            GROUP BY toDate(e."expenseDate", 'UTC')
        "#).bind(start).bind(until).fetch_all(self).await?;
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
