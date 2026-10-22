# NNUE Implementation Session Log

## Session 37: The search check

**Date**: 2026-10-22
**Phase**: Phase 2w
**Objective**: Session 36's handoff question. When YaneuraOu sees a drop attack on our king 3-4
moves before we do, is the cause our evaluation or our search?

> **READ THIS FIRST** (state at the end of this session).
>
> 1. **Both, in two separate stretches** of the q9 two-piece loss
>    (`matches/user_games/2026-10-21_2mai_loss127_windows_q9.txt`, our moves 45-49):
>    - **Moves 45-47: search.** At depth 1 the two engines agree (ours +1151 / +1038 / +826,
>      YaneuraOu +2765 / +978 / +697), so YaneuraOu's evaluation is as blind as ours here. YaneuraOu
>      finds the collapse only by searching, in **0.2-0.6M nodes**. We need **80-90M main nodes**
>      (60 s, 4 threads, depth 17) to get partway: move 47, ours +45, YaneuraOu -377 to -780.
>      With more depth our score falls steadily toward YaneuraOu's at every position, and at move 45
>      we switch to YaneuraOu's S*5d from depth 14 (10 s). Nothing is missing; we get there ~100x
>      slower in nodes (~25x allowing for our counts excluding quiescence).
>    - **Moves 48-49: evaluation.** From move 47 a White lance on 4f, backed by the gold on 4e,
>      attacks our rook on 4g. YaneuraOu at depth 1 scores moves 48 and 49 at +22 / +16; ours says
>      +1227 / +1165. Along YaneuraOu's line after 4g4f 4e4f (rook for lance) our depth-1 score is
>      still +934 for us; YaneuraOu's is about -1,400 to -2,000.
> 2. **Diagnostic toggles that do nothing in the lean search**: `SHOGI_NO_LMR` gives byte-identical
>    results (same nodes) because `lean.rs` never reads `no_lmr`; `SHOGI_NO_SEE_QUIET` only touches the
>    classic orderer. `SHOGI_NO_PRUNING` works but explodes (depth 12 at move 49 did not finish in
>    29 min). Per-rule switches inside `lean.rs` are needed before step (c), "which pruning hides the
>    drop", can be measured.
> 3. **Search side, measured (see "Per-rule switches" below)**: no single pruning rule hides the
>    attack. With YaneuraOu's network inside our search (`yo-diagnostic`), the gap to YaneuraOu's
>    verdict halves (~1,000 -> ~430 cp over 10 collapse positions) and then stops shrinking with
>    nodes (8M 432, 32M 437). Switching off any one rule leaves it equal or worse. Reverse
>    futility's fail-soft return (the raw static eval) produces phantom root fail-highs (755 at
>    depths 7-10 at move 47, contradicted by 400-500 cp on re-search), but returning beta or the
>    midpoint changes the gap by under 100 cp, within noise.
> 4. **NEXT (open)**: the position probes cannot resolve changes under ~100 cp. Options: screen
>    `SHOGI_RFP_RETURN=1/2` in games (4x 250 ms SPRTs, needs a window the user picks); or look for
>    the remaining ~430 cp in what YaneuraOu's tree does that ours does not (extensions, mate
>    search near the king), with the network held fixed.
> 5. Earlier handoff options (before the search measurements):
>    - Search: measure node-efficiency to a verdict on more positions (YaneuraOu nodes vs ours to
>      reach the same score band), then add per-rule toggles to `lean.rs` (LMR, LMP, futility, SEE
>      pruning of quiet moves and drops) and find which one costs the most here.
>    - Evaluation: positions like moves 48-49 (a major piece attacked by a backed lance or pawn,
>      opponent holding drops) are where the net is far off. q9k (1.4% our-game positions) moved the
>      collapse score 100-500 cp without changing moves; a larger share is still untested.

## Measurements (`matches/s37/`)

| Move | played | YaneuraOu 2M | ours d1 | YaneuraOu d1 | ours 60 s (depth) |
|---|---|---|---|---|---|
| 45 | 8g8f | +286 (S*5d) | +1151 | +2765 | +409 (16), S*5d |
| 46 | 5h4g | -175 | +1038 | +978 | +340 (17) |
| 47 | 4h4g | -493 | +826 | +697 | +45 (17) |
| 48 | S*6c | -626 (N*6d) | +1227 | +22 | +56 (14), N*6d |
| 49 | 4g4f | -1653 (P*4h) | +1165 | +16 | -86 (15), 4g4f |

- `probe_q9_45_49.txt`: ours per depth at 1 / 10 / 60 s (4 threads, `engines/usi-engine-stable`, q9).
- `yane_fixed_depth_q9_45_49.txt`, `yane_nodes_per_depth.txt`: YaneuraOu (halfkpvm, 1 thread) at
  fixed depths. The second run kept YaneuraOu's hash between `go`s, so its scores differ from the
  first at the same depth; use it only for node counts.
- `pv_walk_q9_49.txt`: our depth-1 and depth-10 scores along YaneuraOu's line from move 49.
- `toggles_q9.txt`: baseline vs `SHOGI_NO_LMR` at move 49 (identical; see point 2).

Scripts: `scripts/session37/probe_depths.py`, `yane_depths.py`, `pv_walk.py`, `toggle_probe.py`.

## Per-rule switches (`SHOGI_LEAN_OFF`, `SHOGI_RFP_RETURN`)

`src/search/lean.rs` gained two diagnostic env switches, both unset by default:
`SHOGI_LEAN_OFF=rfp,razor,null,hist,lmp,fut,see_quiet,see_capture,lmr,q_delta,q_see` (any subset)
and `SHOGI_RFP_RETURN` (0 static eval, the default; 1 beta; 2 midpoint). Unset, the build matches
`engines/usi-engine-stable` in score, best move and depth at all 10 positions (node counts at the
stop differ by under 0.5%, from the stop being polled).

Test set: the two two-piece losses at their collapse, p9 game moves 41-45 and q9 game moves 45-49
(`scripts/session37/positions.txt`). Reference: YaneuraOu halfkpvm, 1 thread, 2M nodes (+143 to
-1,653). Ours: one thread, `go nodes` budgets (`scripts/session37/probe_off.py`,
`summarize_off.py`). "Gap" = our score minus YaneuraOu's, mean over the 10.

| configuration | 2M gap | 8M gap | 32M gap |
|---|---|---|---|
| ours (default) | 1078 | 976 | 798 |
| ours, any one rule off | 986-1145 | 899-1062 | |
| ours, RFP returns beta / midpoint | 1010 / 1014 | 947 / 932 | |
| **YaneuraOu's network in our search** | **570** | **432** | **437** |
| same, any one rule off (+ check SEE 0, mate1, q losing checks 0) | | 456-690 | |
| same, RFP returns beta / midpoint | 644 / 602 | 471 / 505 | |

- `hist` is a no-op by default (history pruning is behind `SHOGI_LEAN_HIST` bit 4).
- The old `SHOGI_NO_LMR` and `SHOGI_NO_SEE_QUIET` toggles are not read by the lean search.
- Exact 0 scores at move 47 are an aspiration re-search value, not a draw: the hand-repetition rule
  off does not change them, and the best-move line has no repetition. `SHOGI_DIAG_ASPIRATION=1`
  traces the root windows.

Files: `matches/s37/off/`, `yo/`, `yo_off/`, `rfp/`.
