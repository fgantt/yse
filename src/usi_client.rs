//! Synchronous USI client for driving external engines (YaneuraOu, Apery, ...).
//!
//! Distinct from `src/usi.rs`, which implements the USI *server* side of our
//! own engine. This module is the *client* side — it spawns an external USI
//! engine as a subprocess and lets us request evaluations or best moves.
//!
//! Used by the NNUE training pipeline (`src/bin/corpus_gen.rs`) to generate
//! `(position, teacher_eval, outcome)` training tuples.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

#[derive(Debug)]
pub enum UsiError {
    Spawn(std::io::Error),
    Io(std::io::Error),
    EngineClosed,
    ProtocolError(String),
}

impl std::fmt::Display for UsiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UsiError::Spawn(e) => write!(f, "failed to spawn engine: {}", e),
            UsiError::Io(e) => write!(f, "i/o error: {}", e),
            UsiError::EngineClosed => write!(f, "engine stdout closed unexpectedly"),
            UsiError::ProtocolError(s) => write!(f, "protocol error: {}", s),
        }
    }
}

impl std::error::Error for UsiError {}

/// Result of a `go` command: bestmove + most-recent info-line stats.
#[derive(Debug, Clone, Default)]
pub struct SearchResult {
    pub bestmove: String,
    pub ponder: Option<String>,
    pub score_cp: Option<i32>,
    pub score_mate: Option<i32>,
    pub depth: Option<u32>,
    pub nodes: Option<u64>,
    pub time_ms: Option<u64>,
}

impl SearchResult {
    /// Convert the engine's last-reported score into a centipawn value, mapping
    /// mate scores to a large magnitude so downstream training code can treat
    /// them as very-winning/losing evaluations.
    pub fn as_cp_with_mate(&self, mate_magnitude: i32) -> Option<i32> {
        if let Some(m) = self.score_mate {
            let v = mate_magnitude.saturating_sub(m.abs());
            return Some(if m > 0 { v } else { -v });
        }
        self.score_cp
    }
}

/// One line of a MultiPV search: a legal move, and the engine's score for it
/// from the side-to-move's point of view.
///
/// This is the shape a ranking objective needs and a plain `go` cannot give:
/// several *siblings* of one position, scored against each other by the same
/// search. A single `bestmove` says which move is best but nothing about how
/// much better it is than the alternatives.
#[derive(Debug, Clone)]
pub struct RankedMove {
    pub mv: String,
    pub score_cp: Option<i32>,
    pub score_mate: Option<i32>,
    pub depth: u32,
}

impl RankedMove {
    /// Centipawn score, mapping mates to a large magnitude — same convention as
    /// [`SearchResult::as_cp_with_mate`].
    pub fn as_cp_with_mate(&self, mate_magnitude: i32) -> Option<i32> {
        if let Some(m) = self.score_mate {
            let v = mate_magnitude.saturating_sub(m.abs());
            return Some(if m > 0 { v } else { -v });
        }
        self.score_cp
    }
}

pub struct UsiEngine {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    #[allow(dead_code)]
    cwd: PathBuf,
}

impl UsiEngine {
    /// Spawn the engine with its working directory set to `cwd` (needed for
    /// engines that resolve eval/book paths relative to cwd, e.g. YaneuraOu).
    pub fn spawn(path: &Path, cwd: &Path) -> Result<Self, UsiError> {
        let mut child = Command::new(path)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(UsiError::Spawn)?;
        let stdin = child.stdin.take().ok_or(UsiError::EngineClosed)?;
        let stdout = BufReader::new(child.stdout.take().ok_or(UsiError::EngineClosed)?);
        Ok(Self { child, stdin, stdout, cwd: cwd.to_path_buf() })
    }

    pub fn send(&mut self, cmd: &str) -> Result<(), UsiError> {
        writeln!(self.stdin, "{}", cmd).map_err(UsiError::Io)?;
        self.stdin.flush().map_err(UsiError::Io)
    }

    fn read_line(&mut self) -> Result<String, UsiError> {
        let mut line = String::new();
        match self.stdout.read_line(&mut line) {
            Ok(0) => Err(UsiError::EngineClosed),
            Ok(_) => Ok(line),
            Err(e) => Err(UsiError::Io(e)),
        }
    }

    /// Send `usi` and read to `usiok` (ignoring id/option lines).
    pub fn handshake(&mut self) -> Result<(), UsiError> {
        self.send("usi")?;
        loop {
            let line = self.read_line()?;
            if line.trim() == "usiok" {
                return Ok(());
            }
        }
    }

    pub fn set_option(&mut self, name: &str, value: &str) -> Result<(), UsiError> {
        self.send(&format!("setoption name {} value {}", name, value))
    }

    /// Send `isready` and read to `readyok`.
    pub fn isready(&mut self) -> Result<(), UsiError> {
        self.send("isready")?;
        loop {
            let line = self.read_line()?;
            if line.trim() == "readyok" {
                return Ok(());
            }
        }
    }

    pub fn new_game(&mut self) -> Result<(), UsiError> {
        self.send("usinewgame")
    }

    /// Set position via `position startpos moves m1 m2 ...`. Empty moves slice
    /// is equivalent to `position startpos`.
    pub fn set_position_startpos(&mut self, moves: &[String]) -> Result<(), UsiError> {
        self.set_position(None, moves)
    }

    /// `position sfen <sfen> moves ...`, or `position startpos moves ...` when
    /// `sfen` is None. Handicap games start from an SFEN, which is how an
    /// engine far ahead of ours can still be a useful sparring partner.
    pub fn set_position(
        &mut self,
        sfen: Option<&str>,
        moves: &[String],
    ) -> Result<(), UsiError> {
        let mut cmd = match sfen {
            Some(s) => format!("position sfen {}", s),
            None => String::from("position startpos"),
        };
        if !moves.is_empty() {
            cmd.push_str(" moves");
            for m in moves {
                cmd.push(' ');
                cmd.push_str(m);
            }
        }
        self.send(&cmd)
    }

    pub fn go_depth(&mut self, depth: u32) -> Result<SearchResult, UsiError> {
        self.send(&format!("go depth {}", depth))?;
        self.read_search_result()
    }

    /// `go <args>` with arbitrary arguments (e.g. `btime 0 wtime 0 byoyomi 1000`,
    /// `nodes 20000`), returning at `bestmove`.
    pub fn go_with(&mut self, args: &str) -> Result<SearchResult, UsiError> {
        self.send(&format!("go {}", args))?;
        self.read_search_result()
    }

    /// Set how many principal variations the engine reports. `1` restores the
    /// default single-line behaviour.
    pub fn set_multipv(&mut self, k: usize) -> Result<(), UsiError> {
        self.set_option("MultiPV", &k.to_string())
    }

    /// Search to `depth` and return every principal variation the engine
    /// reported, ordered best-first.
    ///
    /// Requires a prior [`set_multipv`] call — with MultiPV at its default of 1
    /// this returns a single-element vector. Info lines arrive once per
    /// iteration per line, so we keep the deepest report for each `multipv`
    /// index and discard bound-only lines, whose scores are not yet resolved.
    pub fn go_multipv(&mut self, depth: u32) -> Result<Vec<RankedMove>, UsiError> {
        self.send(&format!("go depth {}", depth))?;
        let mut best: BTreeMap<usize, RankedMove> = BTreeMap::new();
        loop {
            let line = self.read_line()?;
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("info ") {
                if let Some((idx, ranked)) = parse_multipv_line(rest) {
                    match best.get(&idx) {
                        Some(prev) if prev.depth >= ranked.depth => {}
                        _ => {
                            best.insert(idx, ranked);
                        }
                    }
                }
            } else if trimmed.starts_with("bestmove ") {
                return Ok(best.into_values().collect());
            }
        }
    }

    #[allow(dead_code)]
    pub fn go_movetime(&mut self, ms: u32) -> Result<SearchResult, UsiError> {
        self.send(&format!("go movetime {}", ms))?;
        self.read_search_result()
    }

    fn read_search_result(&mut self) -> Result<SearchResult, UsiError> {
        let mut result = SearchResult::default();
        loop {
            let line = self.read_line()?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Some(rest) = trimmed.strip_prefix("info ") {
                parse_info_line(rest, &mut result);
            } else if let Some(rest) = trimmed.strip_prefix("bestmove ") {
                let parts: Vec<&str> = rest.split_whitespace().collect();
                if parts.is_empty() {
                    return Err(UsiError::ProtocolError(format!("bad bestmove: {}", trimmed)));
                }
                result.bestmove = parts[0].to_string();
                if parts.len() >= 3 && parts[1] == "ponder" {
                    result.ponder = Some(parts[2].to_string());
                }
                return Ok(result);
            }
            // ignore all other lines (id, option, info string, book hits, etc.)
        }
    }

    pub fn quit(mut self) -> Result<(), UsiError> {
        let _ = self.send("quit");
        let _ = self.child.wait();
        Ok(())
    }
}

/// Pull `(multipv index, move + score)` out of one info line.
///
/// Returns `None` for lines that are not a resolved MultiPV report: lines with
/// no `multipv` field, no `pv`, or a `lowerbound`/`upperbound` marker, whose
/// score is a window edge rather than the move's value.
fn parse_multipv_line(rest: &str) -> Option<(usize, RankedMove)> {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut index: Option<usize> = None;
    let mut mv: Option<String> = None;
    let mut score_cp = None;
    let mut score_mate = None;
    let mut depth = 0_u32;
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "lowerbound" | "upperbound" => return None,
            "multipv" if i + 1 < tokens.len() => {
                index = tokens[i + 1].parse::<usize>().ok();
                i += 2;
            }
            "depth" if i + 1 < tokens.len() => {
                depth = tokens[i + 1].parse::<u32>().unwrap_or(0);
                i += 2;
            }
            "score" if i + 2 < tokens.len() => {
                match tokens[i + 1] {
                    "cp" => score_cp = tokens[i + 2].parse::<i32>().ok(),
                    "mate" => score_mate = tokens[i + 2].parse::<i32>().ok(),
                    _ => {}
                }
                i += 3;
            }
            // `pv` is last; the move we want is the one token after it.
            "pv" if i + 1 < tokens.len() => {
                mv = Some(tokens[i + 1].to_string());
                break;
            }
            _ => i += 1,
        }
    }
    let index = index?;
    let mv = mv?;
    if score_cp.is_none() && score_mate.is_none() {
        return None;
    }
    Some((index, RankedMove { mv, score_cp, score_mate, depth }))
}

fn parse_info_line(rest: &str, result: &mut SearchResult) {
    let tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut i = 0;
    while i < tokens.len() {
        match tokens[i] {
            "depth" if i + 1 < tokens.len() => {
                if let Ok(v) = tokens[i + 1].parse::<u32>() {
                    result.depth = Some(v);
                }
                i += 2;
            }
            "score" if i + 2 < tokens.len() => {
                match tokens[i + 1] {
                    "cp" => {
                        if let Ok(v) = tokens[i + 2].parse::<i32>() {
                            result.score_cp = Some(v);
                            result.score_mate = None;
                        }
                    }
                    "mate" => {
                        if let Ok(v) = tokens[i + 2].parse::<i32>() {
                            result.score_mate = Some(v);
                        }
                    }
                    _ => {}
                }
                i += 3;
            }
            "nodes" if i + 1 < tokens.len() => {
                if let Ok(v) = tokens[i + 1].parse::<u64>() {
                    result.nodes = Some(v);
                }
                i += 2;
            }
            "time" if i + 1 < tokens.len() => {
                if let Ok(v) = tokens[i + 1].parse::<u64>() {
                    result.time_ms = Some(v);
                }
                i += 2;
            }
            // tokens we don't care about (seldepth, multipv, nps, hashfull, pv, string, ...)
            _ => {
                i += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multipv_line_yields_the_move_and_its_score() {
        let line = "depth 8 seldepth 9 multipv 2 score cp 102 nodes 17782 nps 1481833 \
                    hashfull 0 time 12 pv 2g2f 7c7d 2h3g";
        let (idx, ranked) = parse_multipv_line(line).expect("should parse");
        assert_eq!(idx, 2);
        assert_eq!(ranked.mv, "2g2f");
        assert_eq!(ranked.score_cp, Some(102));
        assert_eq!(ranked.depth, 8);
    }

    #[test]
    fn multipv_mate_scores_become_a_large_magnitude() {
        let line = "depth 6 multipv 1 score mate 3 pv 5e5d 4c4d";
        let (_, ranked) = parse_multipv_line(line).expect("should parse");
        assert_eq!(ranked.score_mate, Some(3));
        assert_eq!(ranked.as_cp_with_mate(30_000), Some(29_997));

        let line = "depth 6 multipv 4 score mate -2 pv 5e5d";
        let (_, ranked) = parse_multipv_line(line).expect("should parse");
        assert_eq!(ranked.as_cp_with_mate(30_000), Some(-29_998));
    }

    #[test]
    fn bound_lines_are_rejected_because_their_score_is_a_window_edge() {
        let line = "depth 8 multipv 1 score cp 102 lowerbound pv 2g2f";
        assert!(parse_multipv_line(line).is_none());
    }

    #[test]
    fn lines_without_a_pv_or_a_multipv_index_are_rejected() {
        assert!(parse_multipv_line("depth 8 multipv 1 score cp 102 nodes 5").is_none());
        assert!(parse_multipv_line("depth 8 score cp 102 pv 2g2f").is_none());
        assert!(parse_multipv_line("string loading eval file").is_none());
    }
}
