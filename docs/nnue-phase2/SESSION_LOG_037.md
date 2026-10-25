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
> 5. **r9 PASSED its SPRT vs q9 (2026-10-24 13:53): +34.6 ± 27.9 Elo, 321-11-262 after 594 games
>    (297 pairs), LLR +2.95** (`matches/s37/sprt_r9.log`). **Shipped 2026-10-24: `nnue/default.bin` is r9**; q9 is kept
>    as `nnue/default-q9.bin`.
>    Pool 249,492,550 positions (batch 4: 66.996M labelled, 62.258M new, 92.9%); r9 = epoch 1
>    (r_clamp 0.826; epoch 2 0.823). Same held-out games as q9 (`eval_correlation`, ~300k positions
>    each): old pool q9 0.821 / r9 0.825, batch 4 q9 0.818 / r9 0.822. Data volume pays a fourth
>    time, with a smaller step than q9's +173: the LLR wandered between +0.2 and +2.7 for 500 games.
>    Data batch 4 -> r9 -> SPRT vs q9 (`corpus_s37/pipeline.sh`, then `resume.sh`; log
>    `corpus_s37/pipeline.out`). Batch 3's recipe, seeds 390001-14, game ids from
>    4,000,000; label depth 8; pooled with `corpus_s36b/pool_d8.jsonl` into `corpus_s37/pool_d8.jsonl`
>    (~250M expected); `r9` = q9 trained 2 epochs at lr 0.001, seed 51 (`weights/session37/`); SPRT
>    [0, 40] r9 vs q9, same binary, 1 s, 4 threads each, seed 3701 (`matches/s37/sprt_r9.log`).
>    Expected: generation ~7 h, labelling ~7 h, training ~7 h per epoch, SPRT 1-3 h.
>    The trainer now splits validation in place (`nnue_offline_trainer.rs`): the old `partition` held
>    two record arrays at once and would need ~75 GB on this 64 GB machine past 268M positions.
>    Same weights byte for byte as the old trainer on a 200k-position run.
>    **Long-lived YaneuraOu processes slow down ~4x after 1-4 hours** on an otherwise free machine.
>    Batch 4 generation took 14 h (batch 3: 6.4 h) and labelling fell from 270 to ~50 batches/h,
>    in batch 3 too after its first 4 hours. At the same moment a fresh labeller did 4.8 ms per
>    position and the old workers ~19 ms, both using full cores (14.2 cores busy over 30 s; no swap,
>    no thermal warning). `scripts/label_stream.py` now starts a fresh worker and engine per batch
>    (`maxtasksperchild=1`); restarted 2026-10-23 07:46 via `corpus_s37/resume.sh`: 280 batches/h.
>    Cause not found. `usi_match` generation keeps its engines for all 41,686 games and is likely
>    hit the same way; restart its engines periodically before the next batch.
> 6. **RUNNING since 2026-10-24 19:43: data batch 5 -> s9 -> SPRT vs r9** (`corpus_s37b/pipeline.sh`,
>    log `corpus_s37b/pipeline.out`, `corpus_s37b/progress.sh` for a status line). Generation now
>    runs 14 slots x 14 runs of 3,000 games with fresh engines (seeds 410000 + 100*slot + run):
>    fresh engines measured 107 games/min per slot, batch 4's single long runs averaged ~50.
>    Labels from game id 5,000,000; pool with `corpus_s37/pool_d8.jsonl` (~311M expected); s9 = r9
>    trained **1 epoch** (q9 and r9 both kept epoch 1), seed 52; SPRT seed 3702
>    (`matches/s37/sprt_s9.log`). The trainer now sizes its record array from a line count: past
>    2^28 records the doubling step would briefly hold ~39 GB of arrays beside ~25 GB of SFENs.
>    Training also picks up `2c04777` (two-digit hand counts such as 10P now parse; 111 of the first
>    200k pool positions have one, so q9 and r9 trained on a sliver of misparsed hands).
> 7. Earlier handoff options (before the search measurements):
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

## The search side, continued (2026-10-25): cold references and a line tracer

**The YaneuraOu reference was warm.** `scripts/session36/yane_scores.py` searches consecutive
positions in one process, so each starts with the previous move's hash table, as in a game. With a
fresh process per position (`scripts/session37/yane_cold.py`, `matches/s37/yane_cold.jsonl`) its
verdicts are less pessimistic: mean over the 10 positions -301 at 2M nodes (warm: -539), -427 at 8M
(move 47: +43 cold vs -493 warm). Against the cold 8M reference at equal reported nodes, our search
with YaneuraOu's network is ~+320 cp too optimistic (not 430), with our network ~+860. Our node
counts exclude quiescence (~3/4 of our tree) and YaneuraOu's do not, so at equal total work the
search gap is larger than that.

**Line tracer** (`SHOGI_TRACE_LINE="m1 m2 ..."`, stderr, one thread; `src/search/lean.rs` and the
root loop): at each node along a given line it prints table cutoffs, reverse futility, razoring,
null move, the line move's place in the order, its pruning rule or reduction, extension and score,
and cutoffs by other moves. Off, it costs one check per node.

Traces at q9 game move 49 (YaneuraOu's network, 1 thread, 2M nodes, `matches/s37/trace_k49*.txt`):
- This build (with `31f00ed`) finds P*4h within 2M nodes, from depth 12; until then it sat at #53 of
  the root order (searched at full depth, null window).
- YaneuraOu's refutation of 4g4f: our defensive moves along it are late in our order and reduced
  (6h5i #26/135 r=2; P*5h #12-#54/140, pruned by late-move pruning at depth 3 in one iteration;
  6i5h #44/73 r=3), and the attacker, already winning by its evaluation, stops at reverse futility.
- Our preferred defence 6c7d+ (YaneuraOu: +3258 for the attacker): we follow the attack (R*9i,
  B*8g first in order and check-extended); the attacker's first move clears beta at ply 7 and the
  search stops, as alpha-beta should. The root's -943 there is an aspiration bound.
- No rule prunes the attacker's key moves wrongly in these lines. The shortfall is depth per node.

Next on this side: compare tree shape per ply with YaneuraOu's at equal total nodes (where our
nodes go: quiescence share, defensive quiet moves, checking drops), not single rules.
