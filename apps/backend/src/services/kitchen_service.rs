//! Shared pure kitchen transition policy used by tenant workflows.
pub fn valid_transition(from: &str, to: &str) -> bool {
    matches!(
        (from, to),
        ("queued", "preparing")
            | ("preparing", "ready")
            | ("ready", "served")
            | ("queued" | "preparing" | "ready", "cancelled")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progression_and_terminal_states() {
        assert!(valid_transition("queued", "preparing"));
        assert!(valid_transition("preparing", "ready"));
        assert!(valid_transition("ready", "served"));
        for state in ["queued", "preparing", "ready"] {
            assert!(valid_transition(state, "cancelled"));
        }
        for state in ["served", "cancelled"] {
            for next in ["queued", "preparing", "ready", "served", "cancelled"] {
                assert!(!valid_transition(state, next));
            }
        }
        assert!(!valid_transition("queued", "served"));
        assert!(!valid_transition("ready", "preparing"));
    }
}
