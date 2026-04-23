//! Synchronous USI client for driving external engines (YaneuraOu, Apery, ...).
//!
//! Distinct from `src/usi.rs`, which implements the USI *server* side of our
//! own engine. This module is the *client* side — it spawns an external USI
//! engine as a subprocess and lets us request evaluations or best moves.
//!
//! Used by the NNUE training pipeline (`src/bin/corpus_gen.rs`) to generate
//! `(position, teacher_eval, outcome)` training tuples.

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
        let mut cmd = String::from("position startpos");
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
