# NNUE Implementation Session Log

## Session 33: Reductions at Split Nodes (rejected)

**Date**: 2026-10-07 to 2026-10-08
**Phase**: Phase 2w
**Objective**: Session 32's *Next* item 1 -- use more of the idle threads without giving up LMR,
starting with reductions at the PV split's nodes.

> **READ THIS FIRST** (state at the end of this session).
>
> 1. **Stable is unchanged: `11220d2`** (`engines/usi-engine-stable`). Nothing shipped.
> 2. **LMR at split nodes: SPRT FAIL**, −22.8 ± 47.3 (below). Patch kept at
>    `patches/session33/split_node_lmr.patch`, code reverted.
> 3. Session 32's READ THIS FIRST still holds otherwise: the open problem is the evaluation of
>    king danger, judged only by games (six-piece handicap SPRT for anything aimed at the
>    user's games).

---

## LMR at split nodes

A split node (`SplitSearch::split`, `src/search/parallel_search.rs`) searches its first move
alone and the rest as parallel zero-window scouts at full depth. The serial search reduces late
quiet moves, so each split ply adds a ply of unreduced width -- the explanation offered in Session
32 for 3 split plies reaching depth 6 sooner and playing 72 Elo worse.

The patch gives scouts the serial search's base reduction (`calculate_lmr_reduction` with default
parameters, without its adaptive adjustments): from the 4th move (1-based), depth > 2, not in
check, not a capture, promotion or check, `r = min(1 + n/8 + depth/4, 3, depth − 1)`. A reduced
scout scoring above alpha is re-searched as a full-depth scout, then with the full window as
before. Off with `SHOGI_SPLIT_NO_LMR=1`. Smoke-tested: legal moves, NNUE loaded, no overruns.

**Gate:** against stable (`11220d2`), 1 s/move, SPRT [0, 40], seed 3301 (private copies; the
first ~40 games shared the machine with one of the user's GUI games):
**90-5-103 over 198 games, −22.8 ± 47.3, LLR −3.01: FAIL** (`matches/s33_splitlmr_sprt.*`).
Ahead 7-1-3 after 11 games, even at 42, behind from ~80. No illegal moves or crashes; longest
think 980 ms (stable 1,035 ms).

So the 2-ply split does not lose strength by searching its scouts unreduced, or what the
reductions save it gives back in moves the serial search would not have missed. Whether 3 split
plies *with* reductions does better than 2 without is untested; on this result it is not the
first thing to try.

## Method notes

- **Run engine copies where they can find the network.** The first smoke test ran the binaries
  from the scratchpad, where `nnue/default.bin` does not resolve; the engine fell back to
  classical evaluation (`info string NNUE not loaded ... classical evaluation`) and showed
  ±400 cp swings between depths that looked like an evaluation bug. With the network loaded,
  engine and `search_oracle` agree exactly. Private copies now sit next to an `nnue/` symlink.
- **Engine copies read the user's prefs** (`thread_count: 20` in
  `~/Library/Application Support/shogi-vibe/engine_prefs.json`) unless `SHOGI_PREFS_DIR` is set;
  the match above set it to an empty directory.

## Next

1. Session 32's item 2: king-danger evaluation (threat inputs in the network), gated by the
   six-piece handicap SPRT.
2. Session 31's C3 and qlegal, re-gated by SPRT.
3. Other uses of idle threads (YBWC at non-PV nodes) remain open, with lower expectations after
   this result.
