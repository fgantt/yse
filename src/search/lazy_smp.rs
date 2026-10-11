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
//! Half the helpers search one ply deeper than the other half, so the threads
//! do not all fill the table with the same depth. USI `ThreadAlgorithm`
//! (Lazy, Split) selects; when unset, `SHOGI_LAZY_SMP=1` selects Lazy.
//!
//! Not the default. Once the magic tables were found (copied binaries had
//! ray-cast, which choked every thread on two global counters), 16 Lazy
//! threads searched ~10x the nodes of one and reached depth sooner, yet lost
//! to one thread 63-3-76 (-32 Elo) at 1 s/move, while the split at 16 threads
//! went 111-1-88 (+40) against the same opponent. Neutral-set move loss at
//! fixed depth 8 did not separate Lazy from one thread, and 12 threads (the
//! performance cores) were no better than 16, so the cause is not found.
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
            *ENV.get_or_init(|| std::env::var("SHOGI_LAZY_SMP").is_ok_and(|v| v == "1"))
        }
    };
    lazy && lean::lean_enabled()
}

/// Running helper threads. Dropping it stops and joins them, so every return
/// from the search ends them.
pub(super) struct LazyHelpers {
    stop: Arc<AtomicBool>,
    /// Depth of the main thread's last completed iteration.
    main_depth: Arc<std::sync::atomic::AtomicU8>,
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
        let main_depth = Arc::new(std::sync::atomic::AtomicU8::new(0));
        let handles = (0..count)
            .filter_map(|id| {
                let tt = Arc::clone(&main.transposition_table);
                let nnue = main.evaluator.nnue_worker_copy();
                let board = board.clone();
                let captured = captured_pieces.clone();
                let stop = stop.clone();
                let main_depth = main_depth.clone();
                std::thread::Builder::new()
                    .name(format!("lazy-smp-{}", id + 1))
                    .stack_size(64 * 1024 * 1024)
                    .spawn(move || {
                        helper_search(
                            id, tt, nnue, board, captured, player, stop, main_depth, max_depth,
                        );
                    })
                    .ok()
            })
            .collect();
        Self { stop, main_depth, handles }
    }

    /// The main thread finished an iteration at `depth`: helpers behind it
    /// skip ahead.
    pub(super) fn main_completed(&self, depth: u8) {
        self.main_depth.store(depth, Ordering::Relaxed);
    }
}

impl Drop for LazyHelpers {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for h in self.handles.drain(..) {
            let _ = h.join();
        }
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
    stop: Arc<AtomicBool>,
    main_depth: Arc<std::sync::atomic::AtomicU8>,
    max_depth: u8,
) {
    // The engine's own table is replaced by the shared one, so keep it small.
    let mut engine = SearchEngine::new(Some(stop.clone()), 1);
    engine.transposition_table = tt;
    engine.evaluator.install_nnue(nnue);
    engine.time_manager.begin_search(None);
    engine.evaluator.nnue_refresh(&board, player, &captured);
    let offset = (id % 2) as u8;
    let mut depth = 1 + offset;
    while depth <= max_depth && !stop.load(Ordering::Relaxed) {
        // An hour: in effect no limit of its own; the main thread's stop ends it.
        engine.search_at_depth(&mut board, &captured, player, depth, 3_600_000, MIN_SCORE, MAX_SCORE);
        depth = (depth + 1).max(main_depth.load(Ordering::Relaxed) + 1 + offset);
    }
}
