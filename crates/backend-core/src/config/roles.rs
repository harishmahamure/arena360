use std::fmt;

/// Process roles (data-platform §5). The MVP runs all three in one process;
/// production can split them with `ARENA_ROLES`, e.g. `ARENA_ROLES=router`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Roles {
    /// Control plane: tenants, ownership, auth, licensing (global PostgreSQL).
    pub control: bool,
    /// Storage Cell: owns tenant SQLite and DuckDB files and serves business APIs.
    pub cell: bool,
    /// Router: resolves the owner cell and forwards HTTP, SSE, and WebSocket traffic.
    pub router: bool,
}

impl Roles {
    pub const ALL: Self = Self {
        control: true,
        cell: true,
        router: true,
    };

    /// Parses a comma-separated role list such as `control,cell,router`.
    pub fn parse(value: &str) -> Result<Self, String> {
        let mut roles = Self {
            control: false,
            cell: false,
            router: false,
        };
        for name in value.split(',').map(str::trim).filter(|s| !s.is_empty()) {
            match name.to_ascii_lowercase().as_str() {
                "control" => roles.control = true,
                "cell" => roles.cell = true,
                "router" => roles.router = true,
                other => {
                    return Err(format!(
                        "unknown role '{other}'; expected control, cell, or router"
                    ))
                }
            }
        }
        if !(roles.control || roles.cell || roles.router) {
            return Err("at least one role is required".into());
        }
        Ok(roles)
    }
}

impl Default for Roles {
    fn default() -> Self {
        Self::ALL
    }
}

impl fmt::Display for Roles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let names: Vec<&str> = [
            (self.control, "control"),
            (self.cell, "cell"),
            (self.router, "router"),
        ]
        .into_iter()
        .filter_map(|(enabled, name)| enabled.then_some(name))
        .collect();
        f.write_str(&names.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_roles() {
        assert_eq!(Roles::parse("control,cell,router"), Ok(Roles::ALL));
    }

    #[test]
    fn parses_a_subset_ignoring_case_spaces_and_duplicates() {
        let roles = Roles::parse(" Router , router,").unwrap();
        assert_eq!(
            roles,
            Roles {
                control: false,
                cell: false,
                router: true
            }
        );
        assert_eq!(roles.to_string(), "router");
    }

    #[test]
    fn rejects_unknown_and_empty_role_lists() {
        assert!(Roles::parse("cell,worker").is_err());
        assert!(Roles::parse("").is_err());
        assert!(Roles::parse(" , ").is_err());
    }

    #[test]
    fn displays_in_canonical_order() {
        assert_eq!(
            Roles::parse("router,control").unwrap().to_string(),
            "control,router"
        );
    }
}
