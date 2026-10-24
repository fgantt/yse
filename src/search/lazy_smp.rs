//! Lazy SMP for the lean search: with `USI_Threads` above one, helper threads
//! run their own iterative deepening over the whole tree from the root,
//! sharing the main thread's transposition table, while the main thread
//! searches exactly as it would alone and alone chooses the move. The helpers
//! help only through the table: cutoffs, scores and best moves they store.
//!
//! Session 34 measured the root/PV split (`parallel_search`) with the lean
//! search: 20 threads searched ~4x the nodes of one in 10 s but reached the
//! same depth, and lost to one thread 55-2-63 (-23 Elo) at 1 s/move. The lean
//! tree is so narrow that the first move's line is nearly all the work, and
//! below the split plies it ran on one thread.
//!
//! Helpers follow Stockfish 9's depth schedule (`SKIP_SIZE`/`SKIP_PHASE`), and
//! a helper's completed result is played when it is deeper than the main
//! thread's and scores higher. USI `ThreadAlgorithm`
//! (Lazy, Split) selects; when unset, `SHOGI_LAZY_SMP=0` selects the split.
//!
//! The first version lost to one thread 63-3-76 (-32 Elo) at 1 s/move while
//! the split at 16 threads won 111-1-88 (+40). The cause was the 16 MB table
//! (262,144 entries): 16 threads overwrote each other's entries, and reached
//! depth 10 no sooner than one thread. With 256 MB, the skip schedule and the
//! helper's result, Lazy 16 beat the split 16 32-1-11 (+180, SPRT pass).
//!
//! A child module of `search_engine` so it can use the engine's private state.

use super::*;

/// Selected multi-threaded search: 0 = unset (environment decides), 1 = Lazy
/// SMP, 2 = the root/PV split.
static THREAD_MODE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// Select the multi-threaded search for the whole process (USI `ThreadAlgorithm`).
pub fn set_lazy_smp(lazy: bool) {
    THREAD_MODE.store(if lazy { 1 } else { 2 }, Ordering::Relaxed);
}

/// Whether multi-threaded searches use Lazy SMP. Only the lean search has it:
/// the classic search always uses the split.
pub fn lazy_smp_enabled() -> bool {
    let lazy = match THREAD_MODE.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            static ENV: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
            *ENV.get_or_init(|| std::env::var("SHOGI_LAZY_SMP").map_or(true, |v| v != "0"))
        }
    };
    lazy && lean::lean_enabled()
}

/// Depth schedule of helper `id` (Stockfish 9's): helper `i` searches depth `d`
/// only when `(d + PHASE[i]) / SIZE[i]` is even, so helpers spread over depths
/// instead of all searching the main thread's next one.
const SKIP_SIZE: [u8; 20] = [1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4];
const SKIP_PHASE: [u8; 20] = [0, 1, 0, 1, 2, 3, 0, 1, 2, 3, 4, 5, 0, 1, 2, 3, 4, 5, 6, 7];

fn helper_skips(id: usize, depth: u8) -> bool {
    let i = id % SKIP_SIZE.len();
    ((depth + SKIP_PHASE[i]) / SKIP_SIZE[i]) % 2 != 0
}

/// A helper's completed root search: depth, score, move.
type HelperResult = (u8, i32, Move);

/// Running helper threads. Dropping it stops and joins them, so every return
/// from the search ends them.
pub(super) struct LazyHelpers {
    stop: Arc<AtomicBool>,
    /// The deepest completed helper search (ties: the higher score).
    best: Arc<std::sync::Mutex<Option<HelperResult>>>,
    handles: Vec<std::thread::JoinHandle<()>>,
}

impl LazyHelpers {
    /// Start `count` helpers searching from the root position, sharing `main`'s
    /// transposition table and network.
    pub(super) fn start(
        main: &SearchEngine,
        board: &BitboardBoard,
        captured_pieces: &CapturedPieces,
        player: Player,
        count: usize,
        max_depth: u8,
    ) -> Self {
        let stop = Arc::new(AtomicBool::new(false));
        let best = Arc::new(std::sync::Mutex::new(None));
        let handles = (0..count)
            .filter_map(|id| {
                let tt = Arc::clone(&main.transposition_table);
                let nnue = main.evaluator.nnue_worker_copy();
                let board = board.clone();
                let captured = captured_pieces.clone();
                let game_path = main.game_path().to_vec();
                let stop = stop.clone();
                let best = best.clone();
                std::thread::Builder::new()
                    .name(format!("lazy-smp-{}", id + 1))
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        helper_search(
                            id, tt, nnue, board, captured, player, &game_path, stop, best,
                            max_depth,
                        );
                    })
                    .ok()
            })
            .collect();
        Self { stop, best, handles }
    }

    /// Stop the helpers and return a helper result to play in place of the
    /// main thread's (completed to `main_depth` with `main_score`): one that is
    /// deeper and scores higher, as Stockfish 9 chose its thread.
    pub(super) fn finish(mut self, main_depth: u8, main_score: i32) -> Option<(Move, i32)> {
        self.stop_and_join();
        let best = self.best.lock().ok()?.clone()?;
        (best.0 > main_depth && best.1 > main_score).then(|| (best.2, best.1))
    }

    fn stop_and_join(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
    }
}

impl Drop for LazyHelpers {
    fn drop(&mut self) {
        self.stop_and_join();
    }
}

#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
fn helper_search(
    id: usize,
    tt: Arc<crate::search::ThreadSafeTranspositionTable>,
    nnue: Option<crate::evaluation::nnue::NNUEEvaluator>,
    mut board: BitboardBoard,
    captured: CapturedPieces,
    player: Player,
    game_path: &[u64],
    stop: Arc<AtomicBool>,
    best: Arc<std::sync::Mutex<Option<HelperResult>>>,
    max_depth: u8,
) {
    // The engine's own table is replaced by the shared one, so keep it small.
    let mut engine = SearchEngine::new(Some(stop.clone()), 1);
    engine.transposition_table = tt;
    engine.set_game_path(game_path);
    engine.evaluator.install_nnue(nnue);
    engine.time_manager.begin_search(None);
    engine.evaluator.nnue_refresh(&board, player, &captured);
    for depth in 1..=max_depth {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        if helper_skips(id, depth) {
            continue;
        }
        // An hour: in effect no limit of its own; the main thread's stop ends it.
        let result = engine.search_at_depth(
            &mut board, &captured, player, depth, 3_600_000, MIN_SCORE, MAX_SCORE,
        );
        if let Some((mv, score)) = result.filter(|_| !engine.last_search_interrupted()) {
            if let Ok(mut b) = best.lock() {
                if b.as_ref().map_or(true, |x| (depth, score) > (x.0, x.1)) {
                    *b = Some((depth, score, mv));
                }
            }
        }
    }
}
