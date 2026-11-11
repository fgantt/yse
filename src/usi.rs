use crate::ShogiEngine;
use std::io::{self, BufRead, Write};

pub struct UsiHandler {
    engine: ShogiEngine,
}

impl UsiHandler {
    pub fn new() -> Self {
        Self { engine: ShogiEngine::new() }
    }

    pub fn handle_command(&mut self, command_str: &str) -> Vec<String> {
        let parts: Vec<&str> = command_str.trim().split_whitespace().collect();

        if parts.is_empty() {
            return Vec::new();
        }

        if self.engine.is_debug_mode() {
            // TODO: Add proper logging instead of returning debug messages.
        }

        match parts[0] {
            "usi" => self.handle_usi(),
            "isready" => self.handle_isready(),
            "debug" => self.engine.handle_debug(&parts[1..]),
            "position" => self.engine.handle_position(&parts[1..]),
            "go" => self.handle_go(&parts[1..]),
            "stop" => self.engine.handle_stop(),
            "ponderhit" => self.engine.handle_ponderhit(),
            "setoption" => self.engine.handle_setoption(&parts[1..]),
            "usinewgame" => self.engine.handle_usinewgame(),
            "gameover" => self.engine.handle_gameover(&parts[1..]),
            "quit" => Vec::new(), // quit is handled by the caller
            _ => vec![format!("info string Unknown command: {}", parts.join(" "))],
        }
    }

    fn handle_go(&mut self, parts: &[&str]) -> Vec<String> {
        crate::debug_utils::set_search_start_time();
        let limits = GoLimits::parse(parts);
        if limits.mate {
            return vec![self.engine.handle_go_mate(limits.mate_ms)];
        }
        // `go infinite` / `go ponder` must search until `stop`, so no book there.
        if !limits.infinite {
            if let Some(hit) = self.engine.probe_book() {
                let usi = hit.mv.to_usi_string();
                return vec![
                    format!("info string book {} value {} of {} candidates", usi, hit.value, hit.candidates),
                    format!("info depth 1 score cp {} pv {}", hit.value, usi),
                    format!("bestmove {}", usi),
                ];
            }
        }
        let black_to_move = self.engine.current_player == crate::types::Player::Black;
        let time_to_use = limits.allocate_ms(black_to_move, self.engine.move_overhead_ms);
        let depth = limits.depth.unwrap_or(self.engine.depth);

        // The stop flag is reset by the command loop before dispatching `go` (see
        // `run_usi_loop`), so a `stop` that arrives during the search is honoured.
        let best_move = self.engine.get_best_move_with_limits(
            depth,
            time_to_use,
            limits.nodes,
            Some(self.engine.stop_flag.clone()),
        );

        match best_move {
            Some(mv) => vec![format!("bestmove {}", mv.to_usi_string())],
            None => vec!["bestmove resign".to_string()],
        }
    }

    fn handle_usi(&self) -> Vec<String> {
        // The count this engine will use if the GUI sends none: the saved
        // preference, else the CPU count. It used to report the CPU count
        // whatever the preference said (council review, 2026-11-09).
        let thread_count = self.engine.thread_count();
        let parallel_options = self.engine.parallel_search_options();
        vec![
            "id name Yggdrasil".to_string(),
            "id author fgantt (Gemini & Cursor)".to_string(),
            format!(
                "option name USI_Hash type spin default {} min 1 max 1024",
                crate::DEFAULT_HASH_MB
            ),
            "option name USI_Ponder type check default false".to_string(),
            "option name USI_OwnBook type check default true".to_string(),
            "option name UseNNUE type check default true".to_string(),
            "option name MoveOverhead type spin default 100 min 0 max 10000".to_string(),
            format!("option name NNUEFile type string default {}", crate::DEFAULT_NNUE_FILE),
            format!("option name BookFile type string default {}", crate::DEFAULT_BOOK_FILE),
            "option name BookEvalDiff type spin default 0 min 0 max 10000".to_string(),
            "option name BookMaxPly type spin default 999 min 0 max 9999".to_string(),
            format!(
                "option name ParallelEnable type check default {}",
                if parallel_options.enable_parallel { "true" } else { "false" }
            ),
            format!(
                "option name ParallelHash type spin default {} min 1 max 512",
                parallel_options.hash_size_mb
            ),
            format!(
                "option name ParallelMinDepth type spin default {} min 0 max 32",
                parallel_options.min_depth_parallel
            ),
            "option name PSTPreset type combo default Builtin var Builtin var Default var Custom"
                .to_string(),
            "option name PSTPath type string default".to_string(),
            format!(
                "option name ParallelMetrics type check default {}",
                if parallel_options.enable_metrics { "true" } else { "false" }
            ),
            format!(
                "option name YBWCEnable type check default {}",
                if parallel_options.ybwc_enabled { "true" } else { "false" }
            ),
            format!(
                "option name YBWCMinDepth type spin default {} min 0 max 32",
                parallel_options.ybwc_min_depth
            ),
            format!(
                "option name YBWCMinBranch type spin default {} min 1 max 256",
                parallel_options.ybwc_min_branch
            ),
            format!(
                "option name YBWCMaxSiblings type spin default {} min 1 max 256",
                parallel_options.ybwc_max_siblings
            ),
            format!(
                "option name YBWCScalingShallow type spin default {} min 1 max 32",
                parallel_options.ybwc_shallow_divisor
            ),
            format!(
                "option name YBWCScalingMid type spin default {} min 1 max 32",
                parallel_options.ybwc_mid_divisor
            ),
            format!(
                "option name YBWCScalingDeep type spin default {} min 1 max 32",
                parallel_options.ybwc_deep_divisor
            ),
            // Fixed: MaxDepth now allows 0-100 (0 = unlimited/adaptive), default 0
            "option name MaxDepth type spin default 0 min 0 max 100".to_string(),
            "option name EnableTablebase type check default false".to_string(),
            format!("option name USI_Threads type spin default {} min 1 max 32", thread_count),
            "option name SearchAlgorithm type combo default Lean var Lean var Classic".to_string(),
            "option name ThreadAlgorithm type combo default Lazy var Lazy var Split".to_string(),
            // Time Management Options (Task 8.0, 4.0)
            "option name TimeCheckFrequency type spin default 1024 min 1 max 100000".to_string(),
            "option name TimeSafetyMargin type spin default 100 min 0 max 10000".to_string(),
            "option name TimeAllocationStrategy type combo default Adaptive var Equal var \
             Exponential var Adaptive"
                .to_string(),
            "option name EnableTimeBudget type check default true".to_string(),
            "option name EnableCheckOptimization type check default true".to_string(),
            // Aspiration Window Options (Task 7.0)
            "option name EnableAspirationWindows type check default true".to_string(),
            "option name AspirationWindowSize type spin default 25 min 10 max 500".to_string(),
            "option name EnablePositionTypeTracking type check default true".to_string(),
            // Legacy depth option (for backward compatibility, maps to MaxDepth)
            "option name depth type spin default 0 min 0 max 100".to_string(),
            "usiok".to_string(),
        ]
    }

    fn handle_isready(&self) -> Vec<String> {
        let mut response = Vec::new();
        
        // Send NNUE status during isready (standard USI practice for status messages)
        let status = self.engine.nnue_status();
        if !status.is_empty() {
            response.push(format!("info string {}", status));
        }
        let book = self.engine.book_status();
        if !book.is_empty() {
            response.push(format!("info string {}", book));
        }
        
        response.push("readyok".to_string());
        response
    }
}

/// The limits of a USI `go` command.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct GoLimits {
    pub btime: u64,
    pub wtime: u64,
    pub binc: u64,
    pub winc: u64,
    pub byoyomi: u64,
    pub movetime: Option<u64>,
    pub depth: Option<u8>,
    pub nodes: Option<u64>,
    /// `infinite` or `ponder`: search until `stop` (or, pondering, until
    /// `ponderhit` starts the clock; see `run_usi_loop_with`).
    pub infinite: bool,
    /// `ponder`: the search is on the opponent's time.
    pub ponder: bool,
    /// `mate`: a mate search, answered with `checkmate ...`, not `bestmove`.
    pub mate: bool,
    /// The `mate` time limit in ms; `None` for `mate infinite` (until `stop`).
    pub mate_ms: Option<u64>,
}

impl GoLimits {
    pub fn parse(parts: &[&str]) -> Self {
        let mut g = Self::default();
        let mut i = 0;
        while i < parts.len() {
            let next = || parts.get(i + 1).and_then(|v| v.parse::<u64>().ok());
            let mut takes_value = true;
            match parts[i] {
                "btime" => g.btime = next().unwrap_or(0),
                "wtime" => g.wtime = next().unwrap_or(0),
                "binc" => g.binc = next().unwrap_or(0),
                "winc" => g.winc = next().unwrap_or(0),
                "byoyomi" => g.byoyomi = next().unwrap_or(0),
                "movetime" => g.movetime = next(),
                "depth" => g.depth = next().map(|d| d.clamp(1, 99) as u8),
                "nodes" => g.nodes = next(),
                "mate" => {
                    g.mate = true;
                    g.mate_ms = next();
                    takes_value = g.mate_ms.is_some() || parts.get(i + 1) == Some(&"infinite");
                }
                "infinite" => {
                    g.infinite = true;
                    takes_value = false;
                }
                "ponder" => {
                    g.infinite = true;
                    g.ponder = true;
                    takes_value = false;
                }
                _ => takes_value = false,
            }
            i += if takes_value { 2 } else { 1 };
        }
        g
    }

    /// The time a `go ponder` search gets once `ponderhit` arrives: what the
    /// same limits would give without `ponder`, less the same fixed 50 ms the
    /// search keeps back from a timed move (`IterativeDeepening::search`). A
    /// pondering search runs unlimited, so the `ponderhit` timer is its only stop.
    pub fn ponderhit_ms(&self, black_to_move: bool, overhead_ms: u32) -> u32 {
        let own = Self { infinite: false, ponder: false, ..self.clone() };
        own.allocate_ms(black_to_move, overhead_ms).saturating_sub(50).max(1)
    }

    /// Milliseconds to spend on this move.
    ///
    /// * `infinite` / `ponder`: until `stop` (a pondering search: until the
    ///   `ponderhit` timer, see [`Self::ponderhit_ms`]).
    /// * `movetime`: that, less the overhead.
    /// * A clock: about 1/30 of the remaining main time, plus the increment,
    ///   plus byoyomi; never more than a quarter of the main time on top of
    ///   increment and byoyomi, less the overhead. With byoyomi only, the
    ///   byoyomi less the overhead.
    /// * Only `depth` or `nodes`: no time limit.
    /// * Nothing at all: 5 s.
    pub fn allocate_ms(&self, black_to_move: bool, overhead_ms: u32) -> u32 {
        const UNLIMITED: u32 = u32::MAX / 4;
        let overhead = overhead_ms as u64;
        if self.infinite {
            return UNLIMITED;
        }
        if let Some(mt) = self.movetime {
            return mt.saturating_sub(overhead).max(1).min(UNLIMITED as u64) as u32;
        }
        let (time, inc) =
            if black_to_move { (self.btime, self.binc) } else { (self.wtime, self.winc) };
        if time > 0 || inc > 0 || self.byoyomi > 0 {
            let target = time / 30 + inc + self.byoyomi;
            let cap = time / 4 + inc + self.byoyomi;
            return target.min(cap).saturating_sub(overhead).max(10).min(UNLIMITED as u64) as u32;
        }
        if self.depth.is_some() || self.nodes.is_some() {
            return UNLIMITED;
        }
        5000
    }
}

pub fn run_usi_loop() {
    run_usi_loop_with(UsiHandler::new(), io::BufReader::new(io::stdin()), io::stdout());
}

/// What the input thread and the command loop share about the current search.
struct SearchControl {
    /// The number of the `go` being searched (each `go` is numbered as the
    /// input thread reads it). Held while the stop flag is set or reset, so a
    /// `ponderhit` timer stops only its own search, never the next one.
    generation: std::sync::Mutex<u64>,
    /// The number of a `go ponder` still waiting for `ponderhit` or `stop`
    /// (0: none). Set and cleared by the input thread in the order commands
    /// arrive, so a `ponderhit` read before its `go` was dispatched is not lost;
    /// the command loop holds that search's `bestmove` while it is set.
    pondering: std::sync::atomic::AtomicU64,
    /// The time each pondering `go` gets once `ponderhit` arrives, published
    /// by the command loop as it dispatches the `go`: (`go` number, ms).
    ponderhit_ms: std::sync::Mutex<(u64, u32)>,
}

/// [`run_usi_loop`] on any input and output (tests drive it with a pipe).
///
/// Pondering (council review, 2026-11-09: `go ponder` used to be an unlimited
/// search and `ponderhit` only printed a message): `go ponder` searches until
/// `ponderhit` or `stop`. `ponderhit` gives the search the time the same `go`
/// would have had without `ponder`, counted from the `ponderhit`; a search that
/// ends earlier (a proved mate) holds its `bestmove` until `ponderhit` or
/// `stop`, as the protocol requires.
pub fn run_usi_loop_with<R, W>(handler: UsiHandler, input: R, stdout: W)
where
    R: BufRead + Send + 'static,
    W: Write,
{
    run_usi_loop_hooked(handler, input, stdout, LoopHooks::default());
}

/// Test hooks for [`run_usi_loop_with`] (the engine sets none): `after_read`
/// runs on the input thread once it has handled a line, `before_dispatch` in
/// the command loop just before it dispatches one. Tests use them to force an
/// order between the two threads instead of hoping for it.
#[derive(Default)]
pub(crate) struct LoopHooks {
    pub(crate) after_read: Option<Box<dyn Fn(&str) + Send>>,
    pub(crate) before_dispatch: Option<Box<dyn Fn(&str) + Send>>,
}

// One protocol loop: the input thread and the command loop share its state.
#[allow(clippy::too_many_lines)]
pub(crate) fn run_usi_loop_hooked<R, W>(
    mut handler: UsiHandler,
    input: R,
    mut stdout: W,
    hooks: LoopHooks,
) where
    R: BufRead + Send + 'static,
    W: Write,
{
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    let LoopHooks { after_read, before_dispatch } = hooks;

    // Commands are read on their own thread so that `stop` (and `quit`) reach a
    // search in progress: `go` blocks this thread until the search returns, and
    // reading stdin here would only see `stop` after the search had ended, so
    // `go infinite` never returned. Each line carries how many `stop`s had been
    // read before it, so a `stop` read before its `go` was dispatched is not lost
    // when the loop clears the flag to start the search, and its `go` number.
    let stop_flag = handler.engine.stop_flag.clone();
    let stops_seen = Arc::new(AtomicU64::new(0));
    let control = Arc::new(SearchControl {
        generation: std::sync::Mutex::new(0),
        pondering: AtomicU64::new(0),
        ponderhit_ms: std::sync::Mutex::new((0, 0)),
    });
    let (tx, rx) = std::sync::mpsc::channel::<(String, u64, u64)>();
    {
        let stop_flag = stop_flag.clone();
        let stops_seen = stops_seen.clone();
        let control = control.clone();
        std::thread::spawn(move || {
            let mut gos_read = 0u64;
            let stop = |stops_seen: &AtomicU64| {
                stops_seen.fetch_add(1, Ordering::SeqCst);
                stop_flag.store(true, Ordering::SeqCst);
                control.pondering.store(0, Ordering::SeqCst);
            };
            for line in input.lines() {
                let line = line.unwrap_or_default();
                let seq = stops_seen.load(Ordering::SeqCst);
                let cmd = line.trim();
                let words: Vec<&str> = cmd.split_whitespace().collect();
                // `gameover` ends a pondering search too (a GUI need not send
                // `stop` first).
                if cmd == "stop" || cmd == "quit" || words.first() == Some(&"gameover") {
                    stop(&stops_seen);
                }
                if words.first() == Some(&"go") {
                    gos_read += 1;
                    if words.contains(&"ponder") {
                        control.pondering.store(gos_read, Ordering::SeqCst);
                    }
                }
                if cmd == "ponderhit" {
                    let go = control.pondering.swap(0, Ordering::SeqCst);
                    if go != 0 {
                        let hit = std::time::Instant::now();
                        let (stop_flag, control) = (stop_flag.clone(), control.clone());
                        std::thread::spawn(move || ponderhit_timer(go, hit, &control, &stop_flag));
                    }
                }
                if let Some(hook) = &after_read {
                    hook(cmd);
                }
                if tx.send((line, seq, gos_read)).is_err() {
                    return;
                }
            }
            // End of input: as `quit`.
            stop(&stops_seen);
            let seq = stops_seen.load(Ordering::SeqCst);
            let _ = tx.send(("quit".to_string(), seq, gos_read));
        });
    }

    for (command, seq, go_number) in rx {
        let cmd = command.trim();
        if cmd == "quit" {
            break;
        }
        if let Some(hook) = &before_dispatch {
            hook(cmd);
        }
        let parts: Vec<&str> = cmd.split_whitespace().collect();
        let is_go = parts.first() == Some(&"go");
        if is_go {
            let mut g = control.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
            *g = go_number;
            stop_flag.store(false, Ordering::SeqCst);
            if stops_seen.load(Ordering::SeqCst) > seq {
                stop_flag.store(true, Ordering::SeqCst);
            }
            drop(g);
            let limits = GoLimits::parse(&parts[1..]);
            if limits.ponder {
                let black = handler.engine.current_player == crate::types::Player::Black;
                let ms = limits.ponderhit_ms(black, handler.engine.move_overhead_ms);
                *control.ponderhit_ms.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = (go_number, ms);
            }
        }
        // `ponderhit` was handled as it was read (see the input thread).
        if cmd == "ponderhit" {
            continue;
        }

        let output = handler.handle_command(&command);
        if is_go {
            // Hold a pondering search's `bestmove` until `ponderhit` or `stop`.
            while control.pondering.load(Ordering::SeqCst) == go_number {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        }
        for out_line in output {
            if let Err(e) = writeln!(stdout, "{}", out_line) {
                eprintln!("Error writing to stdout: {}", e);
                return;
            }
        }
        if let Err(e) = stdout.flush() {
            eprintln!("Error flushing stdout: {}", e);
            return;
        }
    }
}

/// After `ponderhit` (read at `hit`) for pondering `go` number `go`: waits for
/// that `go`'s time to be published (the `ponderhit` may arrive before the `go`
/// is dispatched), then stops its search that long after `hit`, unless a later
/// `go` has started.
fn ponderhit_timer(
    go: u64,
    hit: std::time::Instant,
    control: &SearchControl,
    stop_flag: &std::sync::atomic::AtomicBool,
) {
    use std::sync::atomic::Ordering;
    let ms = loop {
        let (n, ms) = *control.ponderhit_ms.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        if n == go {
            break ms;
        }
        if *control.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner) > go {
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    };
    let deadline = hit + std::time::Duration::from_millis(u64::from(ms));
    std::thread::sleep(deadline.saturating_duration_since(std::time::Instant::now()));
    let g = control.generation.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if *g == go {
        stop_flag.store(true, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod go_limits_tests {
    use super::GoLimits;

    fn go(s: &str) -> GoLimits {
        GoLimits::parse(&s.split_whitespace().collect::<Vec<_>>())
    }

    #[test]
    fn parses_every_argument() {
        let g = go("btime 60000 wtime 50000 binc 1000 winc 2000 byoyomi 500 depth 7 nodes 12345");
        assert_eq!((g.btime, g.wtime, g.binc, g.winc, g.byoyomi), (60000, 50000, 1000, 2000, 500));
        assert_eq!((g.depth, g.nodes), (Some(7), Some(12345)));
        assert!(go("infinite").infinite && go("ponder btime 1 wtime 1").infinite);
        assert_eq!(go("movetime 750").movetime, Some(750));
        let m = go("mate 3000");
        assert_eq!((m.mate, m.mate_ms, m.infinite), (true, Some(3000), false));
        let m = go("mate infinite");
        assert_eq!((m.mate, m.mate_ms, m.infinite), (true, None, false));
        assert_eq!((go("mate").mate, go("mate").mate_ms), (true, None));
    }

    #[test]
    fn allocates_time_from_the_movers_clock() {
        // byoyomi only: byoyomi less overhead.
        assert_eq!(go("btime 0 wtime 0 byoyomi 1000").allocate_ms(true, 100), 900);
        // main time + increment: time/30 + inc - overhead, for the side to move.
        assert_eq!(go("btime 60000 wtime 30000 binc 1000 winc 0").allocate_ms(true, 100), 2900);
        assert_eq!(go("btime 60000 wtime 30000 binc 1000 winc 0").allocate_ms(false, 100), 900);
        // main time + byoyomi.
        assert_eq!(go("btime 9000 wtime 9000 byoyomi 1000").allocate_ms(true, 100), 1200);
        // Never more than a quarter of the main time on top of inc/byoyomi.
        assert!(go("btime 400 wtime 400").allocate_ms(true, 0) <= 100);
        // movetime, depth-only, infinite, nothing.
        assert_eq!(go("movetime 500").allocate_ms(true, 100), 400);
        assert!(go("depth 5").allocate_ms(true, 100) > 1_000_000);
        assert!(go("nodes 1000").allocate_ms(true, 100) > 1_000_000);
        assert!(go("infinite").allocate_ms(true, 100) > 1_000_000);
        assert_eq!(go("").allocate_ms(true, 100), 5000);
    }

    /// After `ponderhit` a pondering `go` gets what the same limits give
    /// without `ponder`, less the search's fixed 50 ms.
    #[test]
    fn ponderhit_time_is_the_moves_time() {
        assert!(go("ponder btime 0 wtime 0 byoyomi 1000").allocate_ms(true, 100) > 1_000_000);
        assert_eq!(go("ponder btime 0 wtime 0 byoyomi 1000").ponderhit_ms(true, 100), 850);
        assert_eq!(go("ponder btime 60000 wtime 30000 binc 1000").ponderhit_ms(true, 100), 2850);
        assert_eq!(go("ponder btime 0 wtime 0 byoyomi 120").ponderhit_ms(true, 100), 1);
    }
}

#[cfg(test)]
mod setoption_tests {
    use super::UsiHandler;
    use crate::parse_setoption;

    fn split(s: &str) -> Vec<&str> {
        s.split_whitespace().collect()
    }

    #[test]
    fn parses_names_and_values_with_spaces() {
        let p = |s: &str| parse_setoption(&split(s));
        assert_eq!(p("name USI_Hash value 256"), Some(("USI_Hash".into(), "256".into())));
        assert_eq!(
            p("name Book File value /a b/c.db"),
            Some(("Book File".into(), "/a b/c.db".into()))
        );
        assert_eq!(p("name PSTPath value"), Some(("PSTPath".into(), String::new())));
        assert_eq!(p("name Clear Hash"), Some(("Clear Hash".into(), String::new())));
        assert_eq!(p("name value 3"), None);
        assert_eq!(p("USI_Hash value 3"), None);
    }

    #[test]
    fn handles_the_standard_options() {
        let mut usi = UsiHandler::new();
        let out = usi.handle_command("usi");
        for opt in ["USI_Ponder type check", "USI_OwnBook type check default true"] {
            assert!(out.iter().any(|l| l.contains(opt)), "missing {opt}");
        }
        assert!(usi.handle_command("setoption name USI_Ponder value true").is_empty());
        assert!(usi.engine.use_book);
        assert!(usi.handle_command("setoption name USI_OwnBook value false").is_empty());
        assert!(!usi.engine.use_book);
        let out = usi.handle_command("setoption name No Such Option value x y");
        assert_eq!(out, ["info string Unknown option: No Such Option"]);
        // A check option without a value is ignored, not read as false.
        assert!(usi.handle_command("setoption name UseNNUE").is_empty());
        assert!(usi.engine.use_nnue);
    }
}

#[cfg(test)]
mod go_mate_tests {
    use super::UsiHandler;

    fn mate(sfen: &str, limit: &str) -> Vec<String> {
        let mut usi = UsiHandler::new();
        usi.handle_command(&format!("position sfen {sfen}"));
        usi.handle_command(&format!("go mate {limit}"))
    }

    /// A tsume problem: no attacking king, the defender holds the rest.
    #[test]
    fn answers_checkmate_with_the_line() {
        // G*5b, protected by the pawn on 5c, mates the king on 5a.
        let out = mate("4k4/9/4P4/9/9/9/9/9/9 b G2r2b3g4s4n4l17p 1", "5000");
        assert_eq!(out, ["checkmate G*5b"]);
        // A mate in three: the line has all three moves.
        let out = mate("3sks3/9/4S4/9/9/9/9/9/9 b 2G2r2b2g1s4n4l18p 1", "infinite");
        assert_eq!(out.len(), 1);
        assert!(out[0].starts_with("checkmate "), "{out:?}");
        assert_eq!(out[0].split_whitespace().count() % 2, 0, "odd-length line plus the word: {out:?}");
    }

    #[test]
    fn answers_nomate_and_timeout() {
        // Only a pawn check, which the king takes.
        let out = mate("4k4/9/4P4/9/9/9/9/9/9 b 2r2b4g4s4n4l17p 1", "5000");
        assert_eq!(out, ["checkmate nomate"]);
        // The start position: no check at all, so no mate.
        let start = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";
        assert_eq!(mate(start, "1000"), ["checkmate nomate"]);
        // A bare king in the open, with drops to interpose: unsettled in 300 ms.
        let open = "9/9/9/9/4k4/9/9/9/9 b RGSNLPr2b3g3s3n3l17p 1";
        let t = std::time::Instant::now();
        assert_eq!(mate(open, "300"), ["checkmate timeout"]);
        assert!(t.elapsed().as_millis() < 2_000);
        // `mate infinite` runs until `stop`.
        let mut usi = UsiHandler::new();
        usi.handle_command(&format!("position sfen {open}"));
        usi.engine.stop_flag.store(true, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(usi.handle_command("go mate infinite"), ["checkmate timeout"]);
    }
}

#[cfg(test)]
mod ponder_tests {
    use super::{run_usi_loop_with, UsiHandler};
    use std::io::{BufReader, Read, Write};
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// Input fed line by line from the test.
    struct Pipe {
        rx: Receiver<Vec<u8>>,
        buf: Vec<u8>,
    }

    impl Read for Pipe {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.buf.is_empty() {
                match self.rx.recv() {
                    Ok(b) => self.buf = b,
                    Err(_) => return Ok(0),
                }
            }
            let n = out.len().min(self.buf.len());
            out[..n].copy_from_slice(&self.buf[..n]);
            self.buf.drain(..n);
            Ok(n)
        }
    }

    #[derive(Clone, Default)]
    struct Out(Arc<Mutex<Vec<u8>>>);

    impl Write for Out {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Out {
        fn bestmove(&self) -> Option<String> {
            let text = String::from_utf8_lossy(&self.0.lock().unwrap()).to_string();
            text.lines().find(|l| l.starts_with("bestmove")).map(str::to_string)
        }
        fn wait_bestmove(&self, limit: Duration) -> Option<(String, Instant)> {
            let start = Instant::now();
            while start.elapsed() < limit {
                if let Some(b) = self.bestmove() {
                    return Some((b, Instant::now()));
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            None
        }
    }

    fn start() -> (Sender<Vec<u8>>, Out, std::thread::JoinHandle<()>) {
        let (tx, rx) = channel();
        let out = Out::default();
        let o = out.clone();
        let h = std::thread::spawn(move || {
            run_usi_loop_with(UsiHandler::new(), BufReader::new(Pipe { rx, buf: Vec::new() }), o);
        });
        (tx, out, h)
    }

    fn send(tx: &Sender<Vec<u8>>, line: &str) {
        tx.send(format!("{line}\n").into_bytes()).unwrap();
    }

    /// `ponderhit` turns the pondering search into one with the move's time
    /// (it used to keep searching until `stop`).
    #[test]
    fn ponderhit_starts_the_clock() {
        let (tx, out, h) = start();
        send(&tx, "position startpos moves 7g7f");
        send(&tx, "go ponder btime 0 wtime 0 byoyomi 500");
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(out.bestmove(), None, "bestmove while pondering");
        let hit = Instant::now();
        send(&tx, "ponderhit");
        let (b, at) = out.wait_bestmove(Duration::from_secs(5)).expect("bestmove after ponderhit");
        assert!(at.duration_since(hit) >= Duration::from_millis(300), "{b} too early");
        send(&tx, "quit");
        h.join().unwrap();
    }

    /// `go ponder` and `ponderhit` in one write (usually both read before the
    /// `go` is dispatched; `ponderhit_before_dispatch_is_not_lost` forces it).
    #[test]
    fn early_ponderhit_is_not_lost() {
        let (tx, out, h) = start();
        send(&tx, "position startpos moves 7g7f");
        // One write: the input thread reads both lines before the command
        // loop dispatches the `go`.
        tx.send(b"go ponder btime 0 wtime 0 byoyomi 500\nponderhit\n".to_vec()).unwrap();
        let (b, _) = out.wait_bestmove(Duration::from_secs(5)).expect("bestmove after early ponderhit");
        assert!(b.starts_with("bestmove "), "{b}");
        send(&tx, "quit");
        h.join().unwrap();
    }

    /// `ponderhit` handled by the input thread before its `go ponder` is
    /// dispatched starts that search's clock (it used to be dropped). The hooks
    /// force the order: the command loop may not dispatch `go ponder` until the
    /// input thread has handled `ponderhit`.
    #[test]
    fn ponderhit_before_dispatch_is_not_lost() {
        let (hit_tx, hit_rx) = channel::<()>();
        let hooks = super::LoopHooks {
            after_read: Some(Box::new(move |cmd: &str| {
                if cmd == "ponderhit" {
                    let _ = hit_tx.send(());
                }
            })),
            before_dispatch: Some(Box::new(move |cmd: &str| {
                if cmd.starts_with("go ponder") {
                    hit_rx.recv_timeout(Duration::from_secs(5)).expect("ponderhit not read");
                }
            })),
        };
        let (tx, rx) = channel();
        let out = Out::default();
        let o = out.clone();
        let h = std::thread::spawn(move || {
            super::run_usi_loop_hooked(UsiHandler::new(), BufReader::new(Pipe { rx, buf: Vec::new() }), o, hooks);
        });
        send(&tx, "position startpos moves 7g7f");
        send(&tx, "go ponder btime 0 wtime 0 byoyomi 500");
        send(&tx, "ponderhit");
        assert!(out.wait_bestmove(Duration::from_secs(5)).is_some(), "the pondering search never ended");
        send(&tx, "quit");
        h.join().unwrap();
    }

    /// `gameover` ends a pondering search (a GUI need not send `stop` first)
    /// and the loop goes on.
    #[test]
    fn gameover_releases_a_pondering_search() {
        let (tx, out, h) = start();
        send(&tx, "position startpos moves 7g7f");
        send(&tx, "go ponder btime 0 wtime 0 byoyomi 500");
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(out.bestmove(), None);
        send(&tx, "gameover lose");
        assert!(out.wait_bestmove(Duration::from_secs(5)).is_some(), "still pondering after gameover");
        send(&tx, "isready");
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(5)
            && !String::from_utf8_lossy(&out.0.lock().unwrap()).contains("readyok")
        {
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(String::from_utf8_lossy(&out.0.lock().unwrap()).contains("readyok"));
        send(&tx, "quit");
        h.join().unwrap();
    }

    /// End of input while pondering ends the search and the loop.
    #[test]
    fn end_of_input_releases_a_pondering_search() {
        let (tx, out, h) = start();
        send(&tx, "position startpos moves 7g7f");
        send(&tx, "go ponder btime 0 wtime 0 byoyomi 500");
        std::thread::sleep(Duration::from_millis(200));
        drop(tx);
        let (tx2, rx2) = channel();
        std::thread::spawn(move || {
            let _ = tx2.send(h.join().is_ok());
        });
        assert_eq!(rx2.recv_timeout(Duration::from_secs(5)), Ok(true), "loop still running");
        assert!(out.bestmove().is_some());
    }

    /// A pondering search that ends on its own (here a proved mate) holds its
    /// `bestmove` until `stop` (it used to send it at once).
    #[test]
    fn early_result_waits_for_stop() {
        let (tx, out, h) = start();
        send(&tx, "position sfen 4k4/9/4P4/9/9/9/9/9/4K4 b G2r2b3g4s4n4l17p 1");
        send(&tx, "go ponder btime 0 wtime 0 byoyomi 500");
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(out.bestmove(), None, "bestmove while pondering");
        send(&tx, "stop");
        let (b, _) = out.wait_bestmove(Duration::from_secs(5)).expect("bestmove after stop");
        assert_eq!(b, "bestmove G*5b");
        send(&tx, "quit");
        h.join().unwrap();
    }
}
