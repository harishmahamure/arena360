use super::has;
use crate::{dto::JwtUserClaims, error::AppError};

/// Central permission boundary for every panel request. Unmapped paths fail closed.
/// Player/device authentication retains its own scoped handlers and never receives the marker.
pub fn permission(method: &str, path: &str, user: &str) -> Option<String> {
    let p: Vec<_> = path.trim_matches('/').split('/').collect();
    let root = *p.first()?;
    let read = matches!(method, "GET" | "HEAD");
    let last = *p.last()?;
    let action = if read { "read" } else { "write" };
    let exact = match root {
        "access" => {
            if p.get(1) == Some(&"self") {
                return Some(String::new());
            } else if read {
                "access:read"
            } else {
                "access:manage"
            }
        }
        "auth"
            if matches!(path, "/auth/me" | "/auth/refresh" | "/auth/admin-shift-close") =>
        {
            return Some(String::new())
        }
        "branding" => return Some(String::new()),
        "auth" if path == "/auth/register" => "players:write",
        "auth" if path == "/auth/staff-shift" => "shifts:write",
        "notifications" => "notifications:read",
        "realtime" if path == "/realtime" => return Some(String::new()),
        "realtime" => "access:manage",
        "metrics" => "access:manage",
        "activity-log" => "activity:read",
        "stats" if p.get(1) == Some(&"finance") => "finance:read",
        "stats" if p.get(1) == Some(&"staff-dashboard") || p.get(1) == Some(&"usage") => {
            "stats:read"
        }
        "stats" => "finance:read",
        "kiosk-orders" if p.get(1) == Some(&"kitchen") => match p.get(2) {
            Some(&"menu") => "kitchen:manage",
            _ => {
                if read {
                    "kitchen:read"
                } else {
                    "kitchen:write"
                }
            }
        },
        "kiosk-orders" => {
            if read {
                "transactions:read"
            } else {
                "transactions:write"
            }
        }
        "users" if p.get(1).is_some_and(|id| *id == user) && (p.contains(&"totp")) => {
            return Some(String::new())
        }
        "users" if path == "/users/me/avatar" => return Some(String::new()),
        "users" if p.contains(&"totp") => "access:manage",
        "users" if last == "credit-limit" => "credit-limit:write",
        "users" if last == "staff-gaming-allowance" => {
            return Some(format!("staff-gaming-allowance:{action}"))
        }
        "users" => return Some(format!("players:{action}")),
        "cash-registers" if last == "reconcile" => "cash-registers:reconcile",
        "cash-registers" if last == "update-opening" => "cash-registers:adjust_opening",
        "shifts" if last == "force-close" => "shifts:force_close",
        "cash-deposits" if !read && matches!(last, "approve" | "reject") => "cash-deposits:approve",
        "expenses" if !read && matches!(last, "approve" | "reject") => "expenses:approve",
        "expense-categories" => return Some(format!("expenses:{action}")),
        "uploads" => "products:write",
        "inventory" => {
            let section = p.get(1).copied().unwrap_or("");
            if section == "purchase-orders" {
                match (read, last) {
                    (true, _) => "procurement:read",
                    (false, "approve" | "reject") => "procurement:approve",
                    (false, "receipts") => "procurement:receive",
                    _ => "procurement:write",
                }
            } else if section.starts_with("reorder") {
                if read {
                    "inventory:read"
                } else {
                    "inventory:reorder_manage"
                }
            } else if read {
                "inventory:read"
            } else if section == "transfer-requests" {
                if matches!(last, "approve" | "reject" | "fulfill") {
                    "inventory:transfer_fulfill"
                } else {
                    "inventory:transfer_request"
                }
            } else if section == "waste-events" {
                if matches!(last, "approve" | "reject") {
                    "inventory:waste_approve"
                } else {
                    "inventory:waste_record"
                }
            } else {
                "inventory:manage"
            }
        }
        "organizations" => {
            if p.contains(&"locations") {
                if read {
                    "locations:read"
                } else {
                    "locations:manage"
                }
            } else if p.contains(&"pricing-rule-sets") {
                if read {
                    "rules:read"
                } else if matches!(last, "publish" | "rollback") {
                    "rules:publish"
                } else {
                    "rules:edit"
                }
            } else if read {
                "settings:read"
            } else {
                "settings:write"
            }
        }
        "devices" | "plans" | "products" | "sessions" | "transactions" | "player-plans"
        | "units" | "shifts" | "cash-registers" | "cash-deposits" | "credit" | "vendors"
        | "expenses" | "games" | "config" => return Some(format!("{root}:{action}")),
        _ => return None,
    };
    Some(exact.into())
}
pub fn authorize(claims: &JwtUserClaims, method: &str, path: &str) -> Result<(), AppError> {
    if claims.tenantId != crate::models::DEFAULT_ORGANIZATION_ID.to_string()
        && !matches!(
            path.trim_matches('/').split('/').next(),
            Some("access" | "auth" | "organizations" | "realtime" | "stats")
        )
    {
        return Err(AppError::Forbidden(
            "Operational ledger access is limited to its owning venue".into(),
        ));
    }
    // Each upload purpose enforces its own permission in the presign handler.
    if path == "/uploads/presign" {
        return Ok(());
    }
    let needed = permission(method, path, &claims.userId).ok_or_else(|| {
        AppError::Forbidden("This operation has no configured panel permission".into())
    })?;
    if path.starts_with("/organizations/")
        && path.split('/').nth(2) != Some(claims.tenantId.as_str())
    {
        return Err(AppError::Forbidden(
            "Switch to the organization before accessing its resources".into(),
        ));
    }
    if needed.is_empty() || has(claims, &needed) {
        Ok(())
    } else {
        Err(AppError::Forbidden(format!(
            "Permission required: {needed}"
        )))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn specific_actions_do_not_fall_back_to_generic_write() {
        for (method, path, expected) in [
            ("PATCH", "/expenses/123/approve", "expenses:approve"),
            ("POST", "/users/other/totp/setup", "access:manage"),
            (
                "POST",
                "/inventory/purchase-orders/123/receipts",
                "procurement:receive",
            ),
            ("PUT", "/kiosk-orders/kitchen/menu/123", "kitchen:manage"),
            ("GET", "/stats/finance/report", "finance:read"),
            ("PUT", "/access/members/123", "access:manage"),
        ] {
            assert_eq!(permission(method, path, "me").as_deref(), Some(expected));
        }
        assert!(permission("POST", "/new-unmapped-module", "me").is_none());
    }
    #[test]
    fn every_business_route_has_an_explicit_boundary() {
        let source = include_str!("../app.rs");
        for line in source.lines() {
            let line = line.trim();
            let path = if line.starts_with(".route(\"") {
                line.split('"').nth(1)
            } else if line.starts_with('"') && line.ends_with("\",") {
                line.split('"').nth(1)
            } else {
                None
            };
            if let Some(path) = path {
                if path != "/"
                    && path.starts_with('/')
                    && !path.starts_with("/api/docs")
                    && !path.starts_with("/health")
                    && !path.starts_with("/kiosk/")
                    && !path.starts_with("/auth/")
                    && !path.starts_with("/uploads/")
                {
                    assert!(permission("GET", path, "me").is_some(), "Unmapped {path}");
                }
            }
        }
    }
}
