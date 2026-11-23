//! How long to think on one move once the clock has main time left: a soft
//! target from the clock, stretched or shrunk by how the search is going.
//!
//! The clock gives a soft target and a hard ceiling (`GoLimits::soft_ms`,
//! `GoLimits::allocate_ms`). The iteration loop asks [`Stability::start_next`]
//! before each new iteration: it starts one while less than half of the
//! stretched target has gone, since an iteration usually costs more than all
//! the earlier ones together. The target grows while the best move keeps
//! changing or the score is falling against the previous move, and shrinks once
//! the best move has held for many iterations. The hard ceiling still stops a
//! running iteration.
//!
//! With byoyomi only (the 1 s gate) there is no soft target and nothing here
//! runs. `SHOGI_TM_OLD=1` restores the old clock allocation as well.

use crate::types::core::Move;

/// Scores beyond this are mate or rule results; they say nothing about drift.
const ORDINARY_SCORE: i32 = 20_000;

/// Whether `SHOGI_TM_OLD` asks for the allocation before soft targets.
pub fn old_time_manager() -> bool {
    std::env::var("SHOGI_TM_OLD").map(|v| v == "1").unwrap_or(false)
}

/// The best move and score across completed iterations of one search.
#[derive(Debug, Clone)]
pub struct Stability {
    /// Best-move changes, halved every iteration (recent changes count most).
    changes: f32,
    best: Option<Move>,
    /// Completed iterations in a row with the same best move.
    same_for: u32,
    score: Option<i32>,
    /// The score of this side's previous search in this game.
    previous_move_score: Option<i32>,
}

impl Stability {
    pub fn new(previous_move_score: Option<i32>) -> Self {
        Self { changes: 0.0, best: None, same_for: 0, score: None, previous_move_score }
    }

    /// Record a completed iteration's best move and score.
    pub fn record(&mut self, depth: u8, best: &Move, score: i32) {
        self.changes *= 0.5;
        match &self.best {
            Some(old) if old == best => self.same_for += 1,
            Some(_) => {
                // The first few iterations change their minds as a matter of course.
                if depth > 4 {
                    self.changes += 1.0;
                }
                self.same_for = 1;
            }
            None => self.same_for = 1,
        }
        self.best = Some(best.clone());
        self.score = Some(score);
    }

    /// How much to stretch the soft target: 0.5 to 3.
    pub fn factor(&self) -> f32 {
        let unstable = 1.0 + self.changes;
        let falling = match (self.previous_move_score, self.score) {
            (Some(before), Some(now)) if before.abs() < ORDINARY_SCORE && now.abs() < ORDINARY_SCORE => {
                (1.0 + (before - now) as f32 / 400.0).clamp(0.8, 1.6)
            }
            _ => 1.0,
        };
        let settled = if self.same_for >= 8 && self.changes < 0.05 { 0.8 } else { 1.0 };
        (unstable * falling * settled).clamp(0.5, 3.0)
    }

    /// Whether to start another iteration after `elapsed_ms`, with soft target
    /// `soft_ms` and hard ceiling `hard_ms`.
    pub fn start_next(&self, elapsed_ms: u32, soft_ms: u32, hard_ms: u32) -> bool {
        let target = (soft_ms as f32 * self.factor()).min(hard_ms as f32);
        (elapsed_ms as f32) * 2.0 <= target
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv(usi: &str) -> Move {
        use crate::types::core::{PieceType, Player, Position};
        let at = |s: &str| Position::from_usi_string(s).unwrap();
        Move::new_move(at(&usi[..2]), at(&usi[2..4]), PieceType::Pawn, Player::Black, false)
    }

    #[test]
    fn a_settled_search_stops_early_and_a_changing_one_goes_on() {
        let mut settled = Stability::new(None);
        for depth in 1..=7 {
            settled.record(depth, &mv("7g7f"), 50);
        }
        assert_eq!(settled.factor(), 1.0);
        for depth in 8..=10 {
            settled.record(depth, &mv("7g7f"), 50);
        }
        assert!(settled.factor() < 1.0);
        // 45% of the target gone: a settled search stops (0.8 x 1000 / 2 = 400).
        assert!(!settled.start_next(450, 1000, 4000));

        let mut changing = Stability::new(None);
        for depth in 1..=10 {
            changing.record(depth, &mv(if depth % 2 == 0 { "7g7f" } else { "2g2f" }), 50);
        }
        assert!(changing.factor() > 1.5);
        assert!(changing.start_next(700, 1000, 4000));
        // Never past half the hard ceiling.
        assert!(!changing.start_next(2100, 1000, 4000));
    }

    #[test]
    fn a_falling_score_buys_time_and_mates_do_not() {
        let mut falling = Stability::new(Some(300));
        falling.record(8, &mv("7g7f"), 50);
        assert!(falling.factor() > 1.5);
        let mut mated = Stability::new(Some(300));
        mated.record(8, &mv("7g7f"), -29_990);
        assert_eq!(mated.factor(), 1.0);
        let mut rising = Stability::new(Some(0));
        rising.record(8, &mv("7g7f"), 300);
        assert!((rising.factor() - 0.8).abs() < 1e-6);
    }

    #[test]
    fn early_changes_of_mind_do_not_count() {
        let mut s = Stability::new(None);
        s.record(1, &mv("7g7f"), 0);
        s.record(2, &mv("2g2f"), 0);
        s.record(3, &mv("7g7f"), 0);
        assert_eq!(s.factor(), 1.0);
    }
}
