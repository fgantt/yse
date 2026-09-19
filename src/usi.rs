use crate::ShogiEngine;
use num_cpus;
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
        let thread_count = num_cpus::get();
        let parallel_options = self.engine.parallel_search_options();
        vec![
            "id name Yggdrasil".to_string(),
            "id author fgantt (Gemini & Cursor)".to_string(),
            "option name USI_Hash type spin default 16 min 1 max 1024".to_string(),
            "option name UseNNUE type check default true".to_string(),
            "option name MoveOverhead type spin default 100 min 0 max 10000".to_string(),
            format!("option name NNUEFile type string default {}", crate::DEFAULT_NNUE_FILE),
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
            format!("option name USI_Threads type spin default {} min 1 max 32", thread_count),
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
    /// `infinite` or `ponder`: search until `stop`.
    pub infinite: bool,
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
                "infinite" | "ponder" => {
                    g.infinite = true;
                    takes_value = false;
                }
                _ => takes_value = false,
            }
            i += if takes_value { 2 } else { 1 };
        }
        g
    }

    /// Milliseconds to spend on this move.
    ///
    /// * `infinite` / `ponder`: until `stop`.
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
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    let mut handler = UsiHandler::new();
    let mut stdout = io::stdout();

    // Commands are read on their own thread so that `stop` (and `quit`) reach a
    // search in progress: `go` blocks this thread until the search returns, and
    // reading stdin here would only see `stop` after the search had ended, so
    // `go infinite` never returned. Each line carries how many `stop`s had been
    // read before it, so a `stop` read before its `go` was dispatched is not lost
    // when the loop clears the flag to start the search.
    let stop_flag = handler.engine.stop_flag.clone();
    let stops_seen = Arc::new(AtomicU64::new(0));
    let (tx, rx) = std::sync::mpsc::channel::<(String, u64)>();
    {
        let stop_flag = stop_flag.clone();
        let stops_seen = stops_seen.clone();
        std::thread::spawn(move || {
            for line in io::stdin().lock().lines() {
                let line = line.unwrap_or_default();
                let seq = stops_seen.load(Ordering::SeqCst);
                let cmd = line.trim();
                if cmd == "stop" || cmd == "quit" {
                    stops_seen.fetch_add(1, Ordering::SeqCst);
                    stop_flag.store(true, Ordering::SeqCst);
                }
                if tx.send((line, seq)).is_err() {
                    return;
                }
            }
            let seq = stops_seen.load(Ordering::SeqCst);
            let _ = tx.send(("quit".to_string(), seq));
        });
    }

    for (command, seq) in rx {
        let cmd = command.trim();
        if cmd == "quit" {
            break;
        }
        if cmd.split_whitespace().next() == Some("go") {
            stop_flag.store(false, Ordering::SeqCst);
            if stops_seen.load(Ordering::SeqCst) > seq {
                stop_flag.store(true, Ordering::SeqCst);
            }
        }

        let output = handler.handle_command(&command);
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
}
