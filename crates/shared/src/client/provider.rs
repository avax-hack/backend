use std::sync::atomic::{AtomicI32, AtomicU32, Ordering};

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum ProviderId {
    Main,
    Sub1,
    Sub2,
}

impl ProviderId {
    pub fn priority_score(&self) -> i32 {
        match self {
            Self::Main => 30,
            Self::Sub1 => 20,
            Self::Sub2 => 10,
        }
    }
}

pub struct ProviderState {
    pub url: String,
    priority_score: i32,
    score: AtomicI32,
    failure_count: AtomicU32,
    success_count: AtomicU32,
}

impl ProviderState {
    pub fn new(url: &str, id: &ProviderId) -> anyhow::Result<Self> {
        let priority = id.priority_score();
        Ok(Self {
            url: url.to_string(),
            priority_score: priority,
            score: AtomicI32::new(50 + priority),
            failure_count: AtomicU32::new(0),
            success_count: AtomicU32::new(0),
        })
    }

    pub fn score(&self) -> i32 {
        self.score.load(Ordering::Relaxed)
    }

    pub fn record_failure(&mut self) {
        let failures = self.failure_count.fetch_add(1, Ordering::Relaxed) + 1;
        let penalty = match failures {
            1..=2 => 15,
            3..=5 => 30,
            6..=10 => 50,
            _ => 70,
        };
        let base = 50 + self.priority_score;
        let new_score = (base - penalty).max(0);
        self.score.store(new_score, Ordering::Relaxed);
    }

    pub fn record_success(&mut self) {
        self.success_count.fetch_add(1, Ordering::Relaxed);
        // Reset failure count on success so next failure uses fresh penalty
        self.failure_count.store(0, Ordering::Relaxed);
        let current = self.score.load(Ordering::Relaxed);
        let new_score = (current + 2).min(100);
        self.score.store(new_score, Ordering::Relaxed);
    }

    pub fn failure_count(&self) -> u32 {
        self.failure_count.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_id_priority_scores() {
        assert_eq!(ProviderId::Main.priority_score(), 30);
        assert_eq!(ProviderId::Sub1.priority_score(), 20);
        assert_eq!(ProviderId::Sub2.priority_score(), 10);
    }

    #[test]
    fn test_provider_id_equality() {
        assert_eq!(ProviderId::Main, ProviderId::Main);
        assert_ne!(ProviderId::Main, ProviderId::Sub1);
        assert_ne!(ProviderId::Sub1, ProviderId::Sub2);
    }

    #[test]
    fn test_provider_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(ProviderId::Main);
        set.insert(ProviderId::Sub1);
        set.insert(ProviderId::Sub2);
        assert_eq!(set.len(), 3);
        assert!(set.contains(&ProviderId::Main));
    }

    #[test]
    fn test_provider_id_clone() {
        let id = ProviderId::Main;
        let cloned = id.clone();
        assert_eq!(id, cloned);
    }

    #[test]
    fn test_provider_state_initial_score_main() {
        let state = ProviderState::new("http://rpc.example.com", &ProviderId::Main).unwrap();
        // Initial score = 50 + priority_score(30) = 80
        assert_eq!(state.score(), 80);
    }

    #[test]
    fn test_provider_state_initial_score_sub1() {
        let state = ProviderState::new("http://rpc2.example.com", &ProviderId::Sub1).unwrap();
        // Initial score = 50 + 20 = 70
        assert_eq!(state.score(), 70);
    }

    #[test]
    fn test_provider_state_initial_score_sub2() {
        let state = ProviderState::new("http://rpc3.example.com", &ProviderId::Sub2).unwrap();
        // Initial score = 50 + 10 = 60
        assert_eq!(state.score(), 60);
    }

    #[test]
    fn test_provider_state_url() {
        let state = ProviderState::new("http://my-rpc.io", &ProviderId::Main).unwrap();
        assert_eq!(state.url, "http://my-rpc.io");
    }

    #[test]
    fn test_provider_state_record_failure_once() {
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        state.record_failure();
        // 1 failure: penalty=15, base=50+30=80, new_score = max(80-15, 0) = 65
        assert_eq!(state.score(), 65);
        assert_eq!(state.failure_count(), 1);
    }

    #[test]
    fn test_provider_state_record_failure_escalating_penalty() {
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        // base = 50 + 30 (Main priority) = 80

        // 1st failure: penalty 15 -> score = 65
        state.record_failure();
        assert_eq!(state.score(), 65);

        // 2nd failure: penalty 15 -> score = 65
        state.record_failure();
        assert_eq!(state.score(), 65);

        // 3rd failure: penalty 30 -> score = 50
        state.record_failure();
        assert_eq!(state.score(), 50);

        // 4th, 5th failures: still penalty 30 -> score = 50
        state.record_failure();
        assert_eq!(state.score(), 50);
        state.record_failure();
        assert_eq!(state.score(), 50);

        // 6th failure: penalty 50 -> score = 30
        state.record_failure();
        assert_eq!(state.score(), 30);
    }

    #[test]
    fn test_provider_state_record_failure_max_penalty() {
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        // Push past 10 failures
        for _ in 0..15 {
            state.record_failure();
        }
        // penalty = 70, base=80, max(80-70, 0) = 10
        assert_eq!(state.score(), 10);
        assert_eq!(state.failure_count(), 15);
    }

    #[test]
    fn test_provider_state_record_success() {
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        // Initial score = 80
        state.record_success();
        assert_eq!(state.score(), 82); // 80 + 2
    }

    #[test]
    fn test_provider_state_record_success_capped_at_100() {
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        // Initial score = 80, need 10 successes to reach 100
        for _ in 0..20 {
            state.record_success();
        }
        assert_eq!(state.score(), 100);
    }

    #[test]
    fn test_provider_state_recovery_after_failure() {
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        state.record_failure(); // score = 65
        state.record_success(); // score = 67, failure_count reset to 0
        assert_eq!(state.score(), 67);
        assert_eq!(state.failure_count(), 0);
    }

    #[test]
    fn test_provider_state_failure_count_initial() {
        let state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        assert_eq!(state.failure_count(), 0);
    }

    #[test]
    fn test_full_failover_cycle() {
        // Simulate: Main fails -> Sub1 becomes best -> Sub1 fails -> Sub2 becomes best
        let mut main = ProviderState::new("http://main.rpc", &ProviderId::Main).unwrap();
        let mut sub1 = ProviderState::new("http://sub1.rpc", &ProviderId::Sub1).unwrap();
        let sub2 = ProviderState::new("http://sub2.rpc", &ProviderId::Sub2).unwrap();

        // Initial scores: Main=80, Sub1=70, Sub2=60
        assert!(main.score() > sub1.score());
        assert!(sub1.score() > sub2.score());

        // Main fails enough to drop below Sub1 (base=80)
        // 6 failures: penalty=50 -> score=30, below Sub1's 70
        for _ in 0..6 {
            main.record_failure();
        }
        assert!(
            sub1.score() > main.score(),
            "After Main failures, Sub1 (score={}) should be preferred over Main (score={})",
            sub1.score(),
            main.score()
        );

        // Sub1 fails enough to drop below Sub2 (base=70)
        // 6 failures: penalty=50 -> score=20, below Sub2's 60
        for _ in 0..6 {
            sub1.record_failure();
        }
        assert!(
            sub2.score() > sub1.score(),
            "After Sub1 failures, Sub2 (score={}) should be preferred over Sub1 (score={})",
            sub2.score(),
            sub1.score()
        );

        // Sub2 is now the best provider
        assert!(sub2.score() > main.score());
        assert!(sub2.score() > sub1.score());
    }

    #[test]
    fn test_priority_score_preserved_after_failure() {
        // Bug fix: record_failure now uses base = 50 + priority_score
        // Main's priority_score = 30, so base = 80
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        assert_eq!(state.score(), 80);

        state.record_failure(); // 1 failure: penalty=15, score=65
        assert_eq!(state.score(), 65);
        // Priority advantage (30) is preserved in the score
        assert!(state.score() > 50, "Main should stay above base 50 after 1 failure");
    }

    #[test]
    fn test_failure_count_resets_on_success() {
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();

        // Accumulate 5 failures
        for _ in 0..5 {
            state.record_failure();
        }
        assert_eq!(state.failure_count(), 5);

        // One success resets failure_count
        state.record_success();
        assert_eq!(state.failure_count(), 0);

        // Next failure starts fresh at penalty=15
        state.record_failure(); // failure_count=1, penalty=15, base=80, score=65
        assert_eq!(state.score(), 65);
        assert_eq!(state.failure_count(), 1);
    }

    #[test]
    fn test_recovery_cycle_fully_recovers() {
        let mut state = ProviderState::new("http://rpc.test", &ProviderId::Main).unwrap();
        let initial_score = state.score(); // 80

        // Fail several times
        for _ in 0..5 {
            state.record_failure();
        }
        // 5th failure: penalty=30, base=80, score=50
        assert_eq!(state.score(), 50);
        assert!(state.score() < initial_score);

        // Recover with successes (first success resets failure_count)
        // 50 + (25 * 2) = 100 (capped)
        for _ in 0..25 {
            state.record_success();
        }
        assert_eq!(state.score(), 100);
        assert_eq!(state.failure_count(), 0);

        // Next failure uses fresh count (1st), so penalty=15, base=80
        state.record_failure();
        assert_eq!(state.score(), 65);
    }
}
