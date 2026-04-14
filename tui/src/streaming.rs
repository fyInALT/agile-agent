use std::time::Duration;
use std::time::Instant;

const ENTER_QUEUE_DEPTH_LINES: usize = 8;
const ENTER_OLDEST_AGE: Duration = Duration::from_millis(120);
const EXIT_QUEUE_DEPTH_LINES: usize = 2;
const EXIT_OLDEST_AGE: Duration = Duration::from_millis(40);
const EXIT_HOLD: Duration = Duration::from_millis(250);
const REENTER_CATCH_UP_HOLD: Duration = Duration::from_millis(250);
const SEVERE_QUEUE_DEPTH_LINES: usize = 64;
const SEVERE_OLDEST_AGE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ChunkingMode {
    #[default]
    Smooth,
    CatchUp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QueueSnapshot {
    pub(crate) queued_lines: usize,
    pub(crate) oldest_age: Option<Duration>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ChunkingDecision {
    pub(crate) mode: ChunkingMode,
    pub(crate) drain_lines: usize,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct AdaptiveChunkingPolicy {
    mode: ChunkingMode,
    below_exit_threshold_since: Option<Instant>,
    last_catch_up_exit_at: Option<Instant>,
}

impl AdaptiveChunkingPolicy {
    pub(crate) fn decide(
        &mut self,
        snapshot: QueueSnapshot,
        now: Instant,
    ) -> ChunkingDecision {
        if snapshot.queued_lines == 0 {
            self.mode = ChunkingMode::Smooth;
            self.below_exit_threshold_since = None;
            return ChunkingDecision {
                mode: self.mode,
                drain_lines: 1,
            };
        }

        match self.mode {
            ChunkingMode::Smooth => {
                if should_enter_catch_up(snapshot)
                    && (!self.reentry_hold_active(now) || is_severe_backlog(snapshot))
                {
                    self.mode = ChunkingMode::CatchUp;
                    self.below_exit_threshold_since = None;
                    self.last_catch_up_exit_at = None;
                }
            }
            ChunkingMode::CatchUp => {
                if should_exit_catch_up(snapshot) {
                    match self.below_exit_threshold_since {
                        Some(since) if now.saturating_duration_since(since) >= EXIT_HOLD => {
                            self.mode = ChunkingMode::Smooth;
                            self.below_exit_threshold_since = None;
                            self.last_catch_up_exit_at = Some(now);
                        }
                        Some(_) => {}
                        None => self.below_exit_threshold_since = Some(now),
                    }
                } else {
                    self.below_exit_threshold_since = None;
                }
            }
        }

        ChunkingDecision {
            mode: self.mode,
            drain_lines: match self.mode {
                ChunkingMode::Smooth => 1,
                ChunkingMode::CatchUp => snapshot.queued_lines.max(1),
            },
        }
    }

    fn reentry_hold_active(&self, now: Instant) -> bool {
        self.last_catch_up_exit_at
            .is_some_and(|at| now.saturating_duration_since(at) < REENTER_CATCH_UP_HOLD)
    }
}

fn should_enter_catch_up(snapshot: QueueSnapshot) -> bool {
    snapshot.queued_lines >= ENTER_QUEUE_DEPTH_LINES
        || snapshot
            .oldest_age
            .is_some_and(|age| age >= ENTER_OLDEST_AGE)
}

fn should_exit_catch_up(snapshot: QueueSnapshot) -> bool {
    snapshot.queued_lines <= EXIT_QUEUE_DEPTH_LINES
        && snapshot
            .oldest_age
            .is_none_or(|age| age <= EXIT_OLDEST_AGE)
}

fn is_severe_backlog(snapshot: QueueSnapshot) -> bool {
    snapshot.queued_lines >= SEVERE_QUEUE_DEPTH_LINES
        || snapshot
            .oldest_age
            .is_some_and(|age| age >= SEVERE_OLDEST_AGE)
}

#[cfg(test)]
mod tests {
    use super::AdaptiveChunkingPolicy;
    use super::ChunkingMode;
    use super::QueueSnapshot;
    use std::time::Duration;
    use std::time::Instant;

    #[test]
    fn high_queue_depth_enters_catch_up_and_drains_full_backlog() {
        let mut policy = AdaptiveChunkingPolicy::default();

        let decision = policy.decide(
            QueueSnapshot {
                queued_lines: 8,
                oldest_age: Some(Duration::from_millis(10)),
            },
            Instant::now(),
        );

        assert_eq!(decision.mode, ChunkingMode::CatchUp);
        assert_eq!(decision.drain_lines, 8);
    }

    #[test]
    fn stale_queue_enters_catch_up_even_with_small_depth() {
        let mut policy = AdaptiveChunkingPolicy::default();

        let decision = policy.decide(
            QueueSnapshot {
                queued_lines: 2,
                oldest_age: Some(Duration::from_millis(150)),
            },
            Instant::now(),
        );

        assert_eq!(decision.mode, ChunkingMode::CatchUp);
        assert_eq!(decision.drain_lines, 2);
    }

    #[test]
    fn empty_queue_stays_smooth_and_drains_single_line() {
        let mut policy = AdaptiveChunkingPolicy::default();

        let decision = policy.decide(
            QueueSnapshot {
                queued_lines: 0,
                oldest_age: None,
            },
            Instant::now(),
        );

        assert_eq!(decision.mode, ChunkingMode::Smooth);
        assert_eq!(decision.drain_lines, 1);
    }
}
