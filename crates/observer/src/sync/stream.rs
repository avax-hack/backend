use dashmap::DashMap;

use crate::config_local;
use crate::event::core::EventType;

/// Tracks the current block range each event type should poll.
#[derive(Debug, Clone, Copy)]
pub struct BlockRange {
    pub from_block: u64,
    pub to_block: u64,
}

/// Manages per-event-type block progress for the streaming side.
pub struct StreamManager {
    progress: DashMap<EventType, u64>,
}

impl StreamManager {
    pub fn new() -> Self {
        Self {
            progress: DashMap::new(),
        }
    }

    /// Initialize block progress for an event type.
    pub fn set_progress(&self, event_type: EventType, block: u64) {
        self.progress.insert(event_type, block);
    }

    /// Get the next block range to poll for a given event type.
    /// Returns a range from the last processed block + 1 to
    /// min(last + BATCH_SIZE, latest_chain_block).
    pub fn get_range(&self, event_type: EventType, latest_block: u64) -> Option<BlockRange> {
        let entry = self.progress.get(&event_type)?;
        let last_processed = *entry;

        let from_block = last_processed + 1;
        if from_block > latest_block {
            return None;
        }

        let to_block = (from_block + *config_local::BATCH_SIZE - 1).min(latest_block);

        Some(BlockRange {
            from_block,
            to_block,
        })
    }

    /// Advance the progress cursor for an event type after successful processing.
    /// Only moves forward — ignores attempts to set a lower block.
    pub fn advance(&self, event_type: EventType, new_block: u64) {
        self.progress
            .entry(event_type)
            .and_modify(|current| {
                if new_block > *current {
                    *current = new_block;
                }
            })
            .or_insert(new_block);
    }

    /// Get the current block for an event type.
    pub fn current_block(&self, event_type: EventType) -> Option<u64> {
        self.progress.get(&event_type).map(|v| *v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── BlockRange tests ────────────────────────────────────────────

    #[test]
    fn block_range_stores_fields() {
        let range = BlockRange {
            from_block: 10,
            to_block: 20,
        };
        assert_eq!(range.from_block, 10);
        assert_eq!(range.to_block, 20);
    }

    #[test]
    fn block_range_is_copy() {
        let a = BlockRange {
            from_block: 1,
            to_block: 2,
        };
        let b = a; // Copy
        assert_eq!(a.from_block, b.from_block);
        assert_eq!(a.to_block, b.to_block);
    }

    // ── StreamManager::set_progress / current_block tests ───────────

    #[test]
    fn current_block_returns_none_when_unset() {
        let mgr = StreamManager::new();
        assert_eq!(mgr.current_block(EventType::Ido), None);
    }

    #[test]
    fn set_progress_and_current_block() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Ido, 100);
        assert_eq!(mgr.current_block(EventType::Ido), Some(100));
    }

    #[test]
    fn set_progress_overwrites_previous() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Swap, 50);
        mgr.set_progress(EventType::Swap, 200);
        assert_eq!(mgr.current_block(EventType::Swap), Some(200));
    }

    #[test]
    fn set_progress_independent_per_event_type() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Ido, 10);
        mgr.set_progress(EventType::Token, 20);
        assert_eq!(mgr.current_block(EventType::Ido), Some(10));
        assert_eq!(mgr.current_block(EventType::Token), Some(20));
        assert_eq!(mgr.current_block(EventType::Swap), None);
    }

    // ── StreamManager::advance tests ────────────────────────────────

    #[test]
    fn advance_updates_progress() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Lp, 100);
        mgr.advance(EventType::Lp, 150);
        assert_eq!(mgr.current_block(EventType::Lp), Some(150));
    }

    #[test]
    fn advance_works_without_prior_set_progress() {
        let mgr = StreamManager::new();
        mgr.advance(EventType::Price, 42);
        assert_eq!(mgr.current_block(EventType::Price), Some(42));
    }

    // ── StreamManager::get_range tests ──────────────────────────────

    #[test]
    fn get_range_returns_none_when_no_progress() {
        let mgr = StreamManager::new();
        assert!(mgr.get_range(EventType::Ido, 1000).is_none());
    }

    #[test]
    fn get_range_returns_none_when_caught_up() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Ido, 500);
        // latest_block == last_processed => from_block = 501 > 500
        assert!(mgr.get_range(EventType::Ido, 500).is_none());
    }

    #[test]
    fn get_range_returns_range_when_behind() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Ido, 100);
        let range = mgr.get_range(EventType::Ido, 500).unwrap();
        assert_eq!(range.from_block, 101);
        // to_block = min(101 + BATCH_SIZE - 1, 500)
        // default BATCH_SIZE is 100, so to_block = min(200, 500) = 200
        assert_eq!(range.to_block, 200);
    }

    #[test]
    fn get_range_clamped_to_latest_block() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Token, 100);
        // latest is only 110, so to_block should be 110 (< 100 + BATCH_SIZE)
        let range = mgr.get_range(EventType::Token, 110).unwrap();
        assert_eq!(range.from_block, 101);
        assert_eq!(range.to_block, 110);
    }

    #[test]
    fn get_range_from_block_zero() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Swap, 0);
        let range = mgr.get_range(EventType::Swap, 50).unwrap();
        assert_eq!(range.from_block, 1);
        assert!(range.to_block <= 50);
    }

    #[test]
    fn get_range_exactly_one_block_ahead() {
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Price, 99);
        let range = mgr.get_range(EventType::Price, 100).unwrap();
        assert_eq!(range.from_block, 100);
        assert_eq!(range.to_block, 100);
    }

    // ── advance with lower block (documents behavior) ───────────────

    #[test]
    fn advance_prevents_going_backward() {
        // Like mark_completed in ReceiveManager, advance only moves forward.
        let mgr = StreamManager::new();
        mgr.set_progress(EventType::Ido, 200);
        mgr.advance(EventType::Ido, 100);
        assert_eq!(
            mgr.current_block(EventType::Ido),
            Some(200),
            "advance should not go backward"
        );
    }

    // ── Full workflow: set_progress -> get_range -> advance -> get_range ─

    #[test]
    fn full_workflow_cursor_advances_through_blocks() {
        let mgr = StreamManager::new();

        // 1. Initialize progress at block 0
        mgr.set_progress(EventType::Swap, 0);
        assert_eq!(mgr.current_block(EventType::Swap), Some(0));

        // 2. First get_range: should start from block 1
        let range1 = mgr.get_range(EventType::Swap, 500).unwrap();
        assert_eq!(range1.from_block, 1);
        // BATCH_SIZE defaults to 100, so to_block = min(1 + 99, 500) = 100
        assert_eq!(range1.to_block, 100);

        // 3. Advance cursor to to_block after processing
        mgr.advance(EventType::Swap, range1.to_block);
        assert_eq!(mgr.current_block(EventType::Swap), Some(100));

        // 4. Next get_range picks up where we left off
        let range2 = mgr.get_range(EventType::Swap, 500).unwrap();
        assert_eq!(range2.from_block, 101);
        assert_eq!(range2.to_block, 200);

        // 5. Advance again
        mgr.advance(EventType::Swap, range2.to_block);

        // 6. Continue until caught up
        let range3 = mgr.get_range(EventType::Swap, 500).unwrap();
        assert_eq!(range3.from_block, 201);
        assert_eq!(range3.to_block, 300);
    }

    // ── Concurrent access with multiple tokio tasks ─────────────────

    #[tokio::test]
    async fn concurrent_access_different_event_types() {
        use std::sync::Arc;

        let mgr = Arc::new(StreamManager::new());

        // Initialize all event types
        for et in EventType::all() {
            mgr.set_progress(*et, 0);
        }

        // Spawn tasks that independently advance different event types
        let mut handles = Vec::new();
        for et in EventType::all() {
            let mgr = Arc::clone(&mgr);
            let et = *et;
            handles.push(tokio::spawn(async move {
                for block in 1..=50 {
                    mgr.advance(et, block);
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // Each event type should have advanced to 50
        for et in EventType::all() {
            assert_eq!(mgr.current_block(*et), Some(50));
        }
    }

    #[tokio::test]
    async fn concurrent_advance_same_event_type() {
        use std::sync::Arc;

        let mgr = Arc::new(StreamManager::new());
        mgr.set_progress(EventType::Ido, 0);

        // Multiple tasks racing to advance the same event type
        let mut handles = Vec::new();
        for i in 0..10 {
            let mgr = Arc::clone(&mgr);
            handles.push(tokio::spawn(async move {
                // Each task writes a unique block number
                mgr.advance(EventType::Ido, (i + 1) * 100);
            }));
        }

        for h in handles {
            h.await.unwrap();
        }

        // The final value should be one of the written values (last writer wins)
        let block = mgr.current_block(EventType::Ido).unwrap();
        assert!(block > 0 && block <= 1000, "block should be a valid written value, got {block}");
        assert_eq!(block % 100, 0, "block should be a multiple of 100, got {block}");
    }
}
