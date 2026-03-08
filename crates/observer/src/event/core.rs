use std::fmt;

/// A batch of events fetched from a block range.
#[derive(Debug, Clone)]
pub struct EventBatch<T> {
    pub events: Vec<T>,
    pub from_block: u64,
    pub to_block: u64,
}

impl<T> EventBatch<T> {
    pub fn new(events: Vec<T>, from_block: u64, to_block: u64) -> Self {
        Self {
            events,
            from_block,
            to_block,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }
}

/// Identifies the type of on-chain event for block progress tracking.
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq)]
pub enum EventType {
    Ido,
    Token,
    Swap,
    Lp,
    Price,
}

impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Ido => "ido",
            Self::Token => "token",
            Self::Swap => "swap",
            Self::Lp => "lp",
            Self::Price => "price",
        }
    }

    pub fn all() -> &'static [EventType] {
        &[
            EventType::Ido,
            EventType::Token,
            EventType::Swap,
            EventType::Lp,
            EventType::Price,
        ]
    }

    /// Returns the event types this type depends on for ordering.
    pub fn dependencies(&self) -> &'static [EventType] {
        match self {
            Self::Ido => &[],
            Self::Token => &[Self::Ido],
            Self::Swap => &[Self::Ido],
            Self::Lp => &[Self::Ido],
            Self::Price => &[Self::Swap],
        }
    }
}

impl fmt::Display for EventType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── EventBatch tests ────────────────────────────────────────────

    #[test]
    fn event_batch_new_stores_fields() {
        let batch = EventBatch::new(vec![1, 2, 3], 10, 20);
        assert_eq!(batch.events, vec![1, 2, 3]);
        assert_eq!(batch.from_block, 10);
        assert_eq!(batch.to_block, 20);
    }

    #[test]
    fn event_batch_is_empty_when_no_events() {
        let batch: EventBatch<u32> = EventBatch::new(vec![], 0, 0);
        assert!(batch.is_empty());
    }

    #[test]
    fn event_batch_is_not_empty_when_has_events() {
        let batch = EventBatch::new(vec!["a"], 0, 0);
        assert!(!batch.is_empty());
    }

    #[test]
    fn event_batch_len_returns_event_count() {
        let batch = EventBatch::new(vec![10, 20, 30, 40], 5, 8);
        assert_eq!(batch.len(), 4);
    }

    #[test]
    fn event_batch_len_zero_for_empty() {
        let batch: EventBatch<String> = EventBatch::new(vec![], 1, 1);
        assert_eq!(batch.len(), 0);
    }

    // ── EventType::all tests ────────────────────────────────────────

    #[test]
    fn event_type_all_contains_every_variant_without_duplicates() {
        let all = EventType::all();
        assert_eq!(all.len(), 5);
        let mut seen = std::collections::HashSet::new();
        for et in all {
            assert!(seen.insert(et), "duplicate event type: {et:?}");
        }
        assert!(all.contains(&EventType::Ido));
        assert!(all.contains(&EventType::Token));
        assert!(all.contains(&EventType::Swap));
        assert!(all.contains(&EventType::Lp));
        assert!(all.contains(&EventType::Price));
    }

    // ── EventType::as_str / Display tests ───────────────────────────

    #[test]
    fn event_type_as_str_and_display_match() {
        let expected = [
            (EventType::Ido, "ido"),
            (EventType::Token, "token"),
            (EventType::Swap, "swap"),
            (EventType::Lp, "lp"),
            (EventType::Price, "price"),
        ];
        for (et, label) in &expected {
            assert_eq!(et.as_str(), *label);
            assert_eq!(format!("{et}"), *label);
        }
    }

    // ── EventType::dependencies tests ───────────────────────────────

    #[test]
    fn ido_has_no_dependencies() {
        assert!(EventType::Ido.dependencies().is_empty());
    }

    #[test]
    fn token_depends_on_ido() {
        let deps = EventType::Token.dependencies();
        assert_eq!(deps, &[EventType::Ido]);
    }

    #[test]
    fn swap_depends_on_ido() {
        let deps = EventType::Swap.dependencies();
        assert_eq!(deps, &[EventType::Ido]);
    }

    #[test]
    fn lp_depends_on_ido() {
        let deps = EventType::Lp.dependencies();
        assert_eq!(deps, &[EventType::Ido]);
    }

    #[test]
    fn price_depends_on_swap() {
        let deps = EventType::Price.dependencies();
        assert_eq!(deps, &[EventType::Swap]);
    }

    // ── Dependency graph invariants ─────────────────────────────────

    #[test]
    fn no_event_type_depends_on_itself() {
        for et in EventType::all() {
            assert!(
                !et.dependencies().contains(et),
                "{et:?} has a self-dependency"
            );
        }
    }

    #[test]
    fn dependency_graph_has_no_cycles() {
        // Simple cycle detection: verify that following dependencies
        // from any node never revisits the starting node.
        for start in EventType::all() {
            let mut visited = std::collections::HashSet::new();
            let mut stack: Vec<&EventType> = vec![start];
            visited.insert(*start);
            while let Some(current) = stack.pop() {
                for dep in current.dependencies() {
                    assert!(
                        dep != start,
                        "Cycle detected: {start:?} -> ... -> {dep:?}"
                    );
                    if visited.insert(*dep) {
                        stack.push(dep);
                    }
                }
            }
        }
    }
}
