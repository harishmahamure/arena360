//! All report tables are replaced by tenant/location-filtered CTEs before execution.
//! The source SQL is checked-in reporting SQL, never user input. UUIDs are typed.
use std::collections::BTreeSet;
use uuid::Uuid;
#[derive(Clone)]
pub struct ReportScope {
    pub organization_id: Uuid,
    pub locations: Option<Vec<Uuid>>,
}
impl ReportScope {
    pub fn key(&self) -> String {
        let mut ids = self.locations.clone().unwrap_or_default();
        ids.sort();
        format!(
            "{}:{}:{ids:?}",
            self.organization_id,
            self.locations.is_none()
        )
    }
    fn predicate(&self, table: &str) -> String {
        let org = format!("\"organizationId\"=toUUID('{}')", self.organization_id);
        let Some(ids) = &self.locations else {
            return if table == "users" {
                format!("id IN (SELECT \"userId\" FROM organization_memberships WHERE {org})")
            } else {
                org
            };
        };
        let ids = ids
            .iter()
            .map(|id| format!("toUUID('{id}')"))
            .collect::<Vec<_>>()
            .join(",");
        let venue = if ids.is_empty() {
            "IN (SELECT toUUID('00000000-0000-0000-0000-000000000000') WHERE 0)".to_owned()
        } else {
            format!("IN ({ids})")
        };
        let shifts = format!("SELECT id FROM shifts WHERE {org} AND \"venueLocationId\" {venue}");
        let stores = format!(
            "SELECT id FROM inventory_locations WHERE {org} AND \"venueLocationId\" {venue}"
        );
        let transactions =
            format!("SELECT id FROM transactions WHERE {org} AND \"venueLocationId\" {venue}");
        let players=format!("SELECT \"playerId\" FROM transactions WHERE {org} AND \"venueLocationId\" {venue} UNION DISTINCT SELECT b.\"playerId\" FROM usage_sessions s JOIN player_plan_balances b ON b.id=s.\"balanceId\" WHERE s.\"organizationId\"=toUUID('{}') AND s.\"venueLocationId\" {venue}",self.organization_id);
        let condition=match table {
            "transactions"|"usage_sessions"|"shifts"|"inventory_locations" => format!("\"venueLocationId\" {venue}"),
            "devices" => format!("\"locationId\" {venue}"),
            "cash_registers"|"cash_deposits"|"credit_settlements"|"expenses" => format!("\"shiftId\" IN ({shifts})"),
            "stock_receipts"|"stock_waste_events"|"location_stock"|"stock_movements"|"inventory_reorder_rules"=>format!("\"locationId\" IN ({stores})"),
            "purchase_orders"=>format!("\"destinationLocationId\" IN ({stores})"),
            "stock_transfer_requests"=>format!("(\"fromLocationId\" IN ({stores}) OR \"toLocationId\" IN ({stores}))"),
            "transaction_products"|"credit_settlement_items"=>format!("\"transactionId\" IN ({transactions})"),
            "stock_receipt_lines"=>format!("\"receiptId\" IN (SELECT id FROM stock_receipts WHERE {org} AND \"locationId\" IN ({stores}))"),
            "stock_waste_lines"=>format!("\"wasteEventId\" IN (SELECT id FROM stock_waste_events WHERE {org} AND \"locationId\" IN ({stores}))"),
            "player_plan_balances"=>format!("\"playerId\" IN ({players})"),
            "users"=> return format!("id IN ({players})"),
            // Catalog dimensions retain historical names even after availability changes.
            _=>"1".into(),
        };
        format!("{org} AND ({condition})")
    }
    pub fn sql(&self, source: &str) -> String {
        let tables = super::worker::schema();
        let mut used = BTreeSet::new();
        let mut result = String::new();
        let chars: Vec<char> = source.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let c = chars[i];
            if c == '\'' || c == '"' || c == '`' {
                let quote = c;
                result.push(c);
                i += 1;
                while i < chars.len() {
                    let c = chars[i];
                    result.push(c);
                    i += 1;
                    if c == quote {
                        if i < chars.len() && chars[i] == quote {
                            result.push(chars[i]);
                            i += 1;
                        } else {
                            break;
                        }
                    }
                }
            } else if c.is_ascii_alphabetic() || c == '_' {
                let start = i;
                i += 1;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let token: String = chars[start..i].iter().collect();
                if tables.contains_key(&token) {
                    used.insert(token.clone());
                    result.push_str(&format!("scoped_{token}"));
                } else {
                    result.push_str(&token);
                }
            } else {
                result.push(c);
                i += 1;
            }
        }
        if used.is_empty() {
            return result;
        }
        let definitions = used
            .iter()
            .map(|table| {
                format!(
                    "scoped_{table} AS (SELECT * FROM {table} WHERE {})",
                    self.predicate(table)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        let trimmed = result.trim_start();
        if trimmed
            .get(..5)
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("WITH "))
            || trimmed
                .get(..5)
                .is_some_and(|prefix| prefix.eq_ignore_ascii_case("WITH\n"))
        {
            format!("WITH {definitions}, {}", &trimmed[4..])
        } else {
            format!("WITH {definitions} {result}")
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scopes_nested_reports_without_rewriting_literals() {
        let scope = ReportScope {
            organization_id: Uuid::nil(),
            locations: Some(vec![Uuid::from_u128(1)]),
        };
        let sql=scope.sql("WITH totals AS (SELECT count(*) FROM transactions) SELECT 'transactions', \"transactions\" FROM users JOIN totals ON 1=1");
        assert!(sql.contains("FROM scoped_transactions"));
        assert!(sql.contains("FROM scoped_users"));
        assert!(sql.contains("SELECT 'transactions', \"transactions\""));
        assert!(sql.contains("venueLocationId"));
        assert!(sql.contains(",  totals AS"));
    }
    #[test]
    fn cache_identity_distinguishes_all_and_selected_locations() {
        let all = ReportScope {
            organization_id: Uuid::nil(),
            locations: None,
        };
        let empty = ReportScope {
            organization_id: Uuid::nil(),
            locations: Some(vec![]),
        };
        assert_ne!(all.key(), empty.key());
        assert!(!all
            .sql("SELECT * FROM transactions")
            .contains("venueLocationId"));
        assert!(empty.sql("SELECT * FROM transactions").contains("WHERE 0"));
    }
}
