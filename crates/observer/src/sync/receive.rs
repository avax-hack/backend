use dashmap::DashMap;

use crate::event::core::EventType;

/// Manages event processing dependencies and completion tracking.
///
/// Some event types depend on others being caught up before they can process
/// a given block. For example, Token events depend on Ido events because
/// project tokens are created by the IDO contract.
pub struct ReceiveManager {
    completed: DashMap<EventType, u64>,
}

impl ReceiveManager {
    pub fn new() -> Self {
        Self {
            completed: DashMap::new(),
        }
    }

    /// Initialize the completion state for an event type.
    pub fn set_completed(&self, event_type: EventType, block: u64) {
        self.completed.insert(event_type, block);
    }

    /// Check whether all dependencies for `event_type` have been processed
    /// up to at least `block`.
    pub fn can_process(&self, event_type: EventType, block: u64) -> bool {
        let deps = event_type.dependencies();
        if deps.is_empty() {
            return true;
        }

        for dep in deps {
            match self.completed.get(dep) {
                Some(dep_block) if *dep_block >= block => {}
                _ => return false,
            }
        }
        true
    }

    /// Mark an event type as having completed processing up to `block`.
    pub fn mark_completed(&self, event_type: EventType, block: u64) {
        self.completed
            .entry(event_type)
            .and_modify(|current| {
                if block > *current {
                    *current = block;
                }
            })
            .or_insert(block);
    }

    /// Get the last completed block for an event type.
    pub fn completed_block(&self, event_type: EventType) -> Option<u64> {
        self.completed.get(&event_type).map(|v| *v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── set_completed / completed_block tests ───────────────────────

    #[test]
    fn completed_block_returns_none_when_unset() {
        let mgr = ReceiveManager::new();
        assert_eq!(mgr.completed_block(EventType::Ido), None);
    }

    #[test]
    fn set_completed_and_read_back() {
        let mgr = ReceiveManager::new();
        mgr.set_completed(EventType::Ido, 100);
        assert_eq!(mgr.completed_block(EventType::Ido), Some(100));
    }

    #[test]
    fn set_completed_overwrites() {
        let mgr = ReceiveManager::new();
        mgr.set_completed(EventType::Token, 50);
        mgr.set_completed(EventType::Token, 200);
        assert_eq!(mgr.completed_block(EventType::Token), Some(200));
    }

    #[test]
    fn set_completed_independent_per_type() {
        let mgr = ReceiveManager::new();
        mgr.set_completed(EventType::Ido, 10);
        mgr.set_completed(EventType::Swap, 20);
        assert_eq!(mgr.completed_block(EventType::Ido), Some(10));
        assert_eq!(mgr.completed_block(EventType::Swap), Some(20));
        assert_eq!(mgr.completed_block(EventType::Lp), None);
    }

    // ── mark_completed tests ────────────────────────────────────────

    #[test]
    fn mark_completed_inserts_when_absent() {
        let mgr = ReceiveManager::new();
        mgr.mark_completed(EventType::Ido, 50);
        assert_eq!(mgr.completed_block(EventType::Ido), Some(50));
    }

    #[test]
    fn mark_completed_advances_forward() {
        let mgr = ReceiveManager::new();
        mgr.set_completed(EventType::Ido, 100);
        mgr.mark_completed(EventType::Ido, 200);
        assert_eq!(mgr.completed_block(EventType::Ido), Some(200));
    }

    #[test]
    fn mark_completed_does_not_go_backward() {
        let mgr = ReceiveManager::new();
        mgr.set_completed(EventType::Ido, 200);
        mgr.mark_completed(EventType::Ido, 100);
        assert_eq!(mgr.completed_block(EventType::Ido), Some(200));
    }

    #[test]
    fn mark_completed_same_block_is_noop() {
        let mgr = ReceiveManager::new();
        mgr.set_completed(EventType::Token, 150);
        mgr.mark_completed(EventType::Token, 150);
        assert_eq!(mgr.completed_block(EventType::Token), Some(150));
    }

    // ── can_process tests ───────────────────────────────────────────

    #[test]
    fn can_process_ido_always_true_no_deps() {
        let mgr = ReceiveManager::new();
        // Ido has no dependencies, so always processable
        assert!(mgr.can_process(EventType::Ido, 1000));
    }

    #[test]
    fn can_process_token_requires_ido() {
        let mgr = ReceiveManager::new();
        // Token depends on Ido; Ido not set => false
        assert!(!mgr.can_process(EventType::Token, 100));

        // Set Ido to block 99 (less than 100) => still false
        mgr.set_completed(EventType::Ido, 99);
        assert!(!mgr.can_process(EventType::Token, 100));

        // Set Ido to block 100 (equal) => true
        mgr.set_completed(EventType::Ido, 100);
        assert!(mgr.can_process(EventType::Token, 100));
    }

    #[test]
    fn can_process_token_true_when_dep_ahead() {
        let mgr = ReceiveManager::new();
        mgr.set_completed(EventType::Ido, 200);
        assert!(mgr.can_process(EventType::Token, 150));
    }

    #[test]
    fn can_process_swap_requires_ido() {
        let mgr = ReceiveManager::new();
        assert!(!mgr.can_process(EventType::Swap, 50));

        mgr.set_completed(EventType::Ido, 50);
        assert!(mgr.can_process(EventType::Swap, 50));
    }

    #[test]
    fn can_process_lp_requires_ido() {
        let mgr = ReceiveManager::new();
        assert!(!mgr.can_process(EventType::Lp, 50));

        mgr.set_completed(EventType::Ido, 50);
        assert!(mgr.can_process(EventType::Lp, 50));
    }

    #[test]
    fn can_process_price_requires_swap() {
        let mgr = ReceiveManager::new();
        assert!(!mgr.can_process(EventType::Price, 75));

        // Setting Ido is not enough for Price (needs Swap)
        mgr.set_completed(EventType::Ido, 100);
        assert!(!mgr.can_process(EventType::Price, 75));

        mgr.set_completed(EventType::Swap, 75);
        assert!(mgr.can_process(EventType::Price, 75));
    }

    #[test]
    fn can_process_at_block_zero() {
        let mgr = ReceiveManager::new();
        mgr.set_completed(EventType::Ido, 0);
        assert!(mgr.can_process(EventType::Token, 0));
    }

    // ── Full dependency chain integration test ──────────────────────

    #[test]
    fn full_dependency_chain_ido_token_swap_price() {
        let mgr = ReceiveManager::new();
        let block = 100;

        // Initially nothing can process except Ido (no deps)
        assert!(mgr.can_process(EventType::Ido, block));
        assert!(!mgr.can_process(EventType::Token, block));
        assert!(!mgr.can_process(EventType::Swap, block));
        assert!(!mgr.can_process(EventType::Lp, block));
        assert!(!mgr.can_process(EventType::Price, block));

        // Complete Ido -> unblocks Token, Swap, Lp (they all depend on Ido)
        mgr.mark_completed(EventType::Ido, block);
        assert!(mgr.can_process(EventType::Token, block));
        assert!(mgr.can_process(EventType::Swap, block));
        assert!(mgr.can_process(EventType::Lp, block));
        // Price depends on Swap, NOT on Ido directly, so still blocked
        assert!(
            !mgr.can_process(EventType::Price, block),
            "Price depends on Swap, not Ido; completing Ido should NOT unblock Price"
        );

        // Complete Token -> does NOT unblock Price (Price depends on Swap)
        mgr.mark_completed(EventType::Token, block);
        assert!(
            !mgr.can_process(EventType::Price, block),
            "Completing Token should NOT unblock Price; Price depends on Swap"
        );

        // Complete Swap -> unblocks Price
        mgr.mark_completed(EventType::Swap, block);
        assert!(
            mgr.can_process(EventType::Price, block),
            "Price should be unblocked once Swap is complete"
        );
    }

    #[test]
    fn dependency_chain_partial_block_progress() {
        // Verify that dependencies must be at the SAME block, not just present
        let mgr = ReceiveManager::new();

        mgr.mark_completed(EventType::Ido, 50);
        // Token at block 100 needs Ido at >= 100
        assert!(
            !mgr.can_process(EventType::Token, 100),
            "Token(100) should be blocked because Ido is only at 50"
        );
        assert!(
            mgr.can_process(EventType::Token, 50),
            "Token(50) should be processable because Ido is at 50"
        );
        assert!(
            mgr.can_process(EventType::Token, 30),
            "Token(30) should be processable because Ido(50) >= 30"
        );
    }
}
