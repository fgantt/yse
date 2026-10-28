# NNUE Implementation Session Log

## Session 37: The search check, three data batches, and label depth

**Date**: 2026-10-22 to 2026-10-28
**Phase**: Phase 2w
**Objective**: Session 36's question (search or evaluation in the drop-attack losses), then more
even-game data (batches 4-6) and whether deeper labels help.

> **READ THIS FIRST** (state at the end of this session, 2026-10-28).
>
> 0. **Shipped network: `nnue/default.bin` is s9** (2026-10-25; 312M even-game positions, depth-8
>    YaneuraOu labels; r9 kept as `nnue/default-r9.bin`, q9 as `nnue/default-q9.bin`). Nothing is
>    running; the machine is free. On disk (ignored): pools `corpus_s37c/pool_d8.jsonl` and
>    `pool_d10.jsonl` (375M each), networks `weights/session37c/t8.bin`, `t10.bin`.
> 1. **Data volume is running out of steam.** Batch 4 (r9) +35 vs q9, batch 5 (s9) **+80** vs r9,
>    batch 6 (t8) **+27.9 ± 25.8** vs s9, inconclusive at the 600-game cap (not shipped). Held-out
>    pearson has sat at ~0.826 since r9 whatever data is added, and +80 came with a 0.001 change,
>    so held-out correlation does not predict game strength at this level.
> 2. **Deeper labels did not help.** Depth-8 labels flatter the side to move (+37 cp overall, +60
>    to +80 with many pieces in hand, **+207** in the contested stretches of the user's losses) and
>    depth 10 halves that bias, but t10 (batch 6 at depth 10) vs t8 (same positions at depth 8):
>    **FAIL, -18.8 ± 44.5**, at ~3x the labelling cost. t10 vs s9 stopped at ~+12 (531 games).
> 3. **Search, measured but not solved.** On 10 collapse positions from the user's losses,
>    evaluation is about half the gap to YaneuraOu; with YaneuraOu's network inside our search we
>    are still ~320 cp too optimistic at equal reported nodes (and our counts exclude quiescence,
>    ~3/4 of our tree). No single pruning rule explains it (per-rule switches `SHOGI_LEAN_OFF`,
>    line tracer `SHOGI_TRACE_LINE`). Compare against YaneuraOu with a **cold** table
>    (`scripts/session37/yane_cold.py`); the warm reference overstated the gap by ~240 cp.
> 4. **The user's losses** (q9 and s9, two-piece handicap, `matches/user_games/2026-10-2*`): a
>    long stretch of evaluation optimism (800-1,300 cp above YaneuraOu) and a drop attack we see
>    2-3 moves late; more time sometimes finds the move but still misjudges the score. The GUI's
>    "YaneuraOu NNUE V9.00Git 64APPLEM1 TOURNAMENT" is the halfkpvm build; the Suisho5 copy reports
>    the same USI id but is a separate GUI entry ("YaneuraOu-Suisho5").
> 5. **NEXT: research before choosing (the user's direction, 2026-10-28).** Do not jump to a
>    conclusion. First survey the techniques used across the field (YaneuraOu and the Stockfish
>    lineage, other shogi engines such as Suisho, Elmo, Apery and dlshogi, published write-ups on
>    NNUE training and architectures, search, mate search, parameter tuning such as SPSA, data
>    generation); then reason carefully about what our measurements say; double-check the plan
>    (e.g. an independent review of it); only then pick an approach. Candidate directions, none
>    decided: search efficiency (mate search near the king, move ordering, tuned pruning
>    constants); automatic tuning (SPSA); raw speed (~2.3x fewer nodes per second per thread than
>    YaneuraOu counting all nodes); how the training data is made (our 375M teacher-labelled
>    positions vs engines trained on billions of their own positions with game results); a larger
>    network (ours is the same size class as YaneuraOu's halfkpvm 256x2-32-32 and Suisho5's
>    halfKP 256, so size alone does not explain the gap). Study their ideas only; never copy code,
>    constants or weights.
> 6. **Operations learned this session** (all in the notes below): long-lived YaneuraOu processes
>    slow ~4x (fresh engines per 3,000-game chunk and per labelling batch); with the user away
>    after a restart macOS throttled the pipeline to 3-20% speed until `caffeinate -u`
>    (`corpus_s37c/stay_active.sh`); one position hangs YaneuraOu at depth 10 (the labeller now
>    drops that game's tail); the trainer needs ~47 GB at 375M positions, so do not label and
>    train at once beside other large apps; two 4-thread SPRTs fit the 16 cores side by side;
>    long sleeps inside background watches stretch, so reporters wake every 20 s.

## Running notes (in order, as written during the session)

1. **Both, in two separate stretches** of the q9 two-piece loss
   (`matches/user_games/2026-10-21_2mai_loss127_windows_q9.txt`, our moves 45-49):
   - **Moves 45-47: search.** At depth 1 the two engines agree (ours +1151 / +1038 / +826,
     YaneuraOu +2765 / +978 / +697), so YaneuraOu's evaluation is as blind as ours here. YaneuraOu
     finds the collapse only by searching, in **0.2-0.6M nodes**. We need **80-90M main nodes**
     (60 s, 4 threads, depth 17) to get partway: move 47, ours +45, YaneuraOu -377 to -780.
     With more depth our score falls steadily toward YaneuraOu's at every position, and at move 45
     we switch to YaneuraOu's S*5d from depth 14 (10 s). Nothing is missing; we get there ~100x
     slower in nodes (~25x allowing for our counts excluding quiescence).
   - **Moves 48-49: evaluation.** From move 47 a White lance on 4f, backed by the gold on 4e,
     attacks our rook on 4g. YaneuraOu at depth 1 scores moves 48 and 49 at +22 / +16; ours says
     +1227 / +1165. Along YaneuraOu's line after 4g4f 4e4f (rook for lance) our depth-1 score is
     still +934 for us; YaneuraOu's is about -1,400 to -2,000.
2. **Diagnostic toggles that do nothing in the lean search**: `SHOGI_NO_LMR` gives byte-identical
   results (same nodes) because `lean.rs` never reads `no_lmr`; `SHOGI_NO_SEE_QUIET` only touches the
   classic orderer. `SHOGI_NO_PRUNING` works but explodes (depth 12 at move 49 did not finish in
   29 min). Per-rule switches inside `lean.rs` are needed before step (c), "which pruning hides the
   drop", can be measured.
3. **Search side, measured (see "Per-rule switches" below)**: no single pruning rule hides the
   attack. With YaneuraOu's network inside our search (`yo-diagnostic`), the gap to YaneuraOu's
   verdict halves (~1,000 -> ~430 cp over 10 collapse positions) and then stops shrinking with
   nodes (8M 432, 32M 437). Switching off any one rule leaves it equal or worse. Reverse
   futility's fail-soft return (the raw static eval) produces phantom root fail-highs (755 at
   depths 7-10 at move 47, contradicted by 400-500 cp on re-search), but returning beta or the
   midpoint changes the gap by under 100 cp, within noise.
4. **NEXT (open)**: the position probes cannot resolve changes under ~100 cp. Options: screen
   `SHOGI_RFP_RETURN=1/2` in games (4x 250 ms SPRTs, needs a window the user picks); or look for
   the remaining ~430 cp in what YaneuraOu's tree does that ours does not (extensions, mate
   search near the king), with the network held fixed.
5. **r9 PASSED its SPRT vs q9 (2026-10-24 13:53): +34.6 ± 27.9 Elo, 321-11-262 after 594 games
   (297 pairs), LLR +2.95** (`matches/s37/sprt_r9.log`). **Shipped 2026-10-24: `nnue/default.bin` is r9**; q9 is kept
   as `nnue/default-q9.bin`.
   Pool 249,492,550 positions (batch 4: 66.996M labelled, 62.258M new, 92.9%); r9 = epoch 1
   (r_clamp 0.826; epoch 2 0.823). Same held-out games as q9 (`eval_correlation`, ~300k positions
   each): old pool q9 0.821 / r9 0.825, batch 4 q9 0.818 / r9 0.822. Data volume pays a fourth
   time, with a smaller step than q9's +173: the LLR wandered between +0.2 and +2.7 for 500 games.
   Data batch 4 -> r9 -> SPRT vs q9 (`corpus_s37/pipeline.sh`, then `resume.sh`; log
   `corpus_s37/pipeline.out`). Batch 3's recipe, seeds 390001-14, game ids from
   4,000,000; label depth 8; pooled with `corpus_s36b/pool_d8.jsonl` into `corpus_s37/pool_d8.jsonl`
   (~250M expected); `r9` = q9 trained 2 epochs at lr 0.001, seed 51 (`weights/session37/`); SPRT
   [0, 40] r9 vs q9, same binary, 1 s, 4 threads each, seed 3701 (`matches/s37/sprt_r9.log`).
   Expected: generation ~7 h, labelling ~7 h, training ~7 h per epoch, SPRT 1-3 h.
   The trainer now splits validation in place (`nnue_offline_trainer.rs`): the old `partition` held
   two record arrays at once and would need ~75 GB on this 64 GB machine past 268M positions.
   Same weights byte for byte as the old trainer on a 200k-position run.
   **Long-lived YaneuraOu processes slow down ~4x after 1-4 hours** on an otherwise free machine.
   Batch 4 generation took 14 h (batch 3: 6.4 h) and labelling fell from 270 to ~50 batches/h,
   in batch 3 too after its first 4 hours. At the same moment a fresh labeller did 4.8 ms per
   position and the old workers ~19 ms, both using full cores (14.2 cores busy over 30 s; no swap,
   no thermal warning). `scripts/label_stream.py` now starts a fresh worker and engine per batch
   (`maxtasksperchild=1`); restarted 2026-10-23 07:46 via `corpus_s37/resume.sh`: 280 batches/h.
   Cause not found. `usi_match` generation keeps its engines for all 41,686 games and is likely
   hit the same way; restart its engines periodically before the next batch.
6. **s9 PASSED its SPRT vs r9 (2026-10-25): +79.5 ± 58.1 Elo, 72-3-45 after 120 games
   (60 pairs), LLR +3.05** (`matches/s37/sprt_s9.log`). **Shipped 2026-10-25: `nnue/default.bin` is s9**; r9 is
   kept as `nnue/default-r9.bin`.
   Pool 312,188,512 (batch 5: 67.51M labelled, 62.70M new); one epoch, 8.5 h, r_clamp 0.818 on
   its own split. On the same held-out games s9 leads r9 by only 0.001 pearson (old pool
   0.825 / 0.826, batch 4 0.822 / 0.823, batch 5 0.824 / 0.825), yet +80 Elo in games: held-out
   correlation is a poor proxy at this level. Stage times: generation 6.4 h (chunked runs),
   labelling 4.5 h, pool 5 min, training 8.5 h, SPRT 3 h.
   Data batch 5 -> s9 -> SPRT vs r9 (`corpus_s37b/pipeline.sh`,
   log `corpus_s37b/pipeline.out`, `corpus_s37b/progress.sh` for a status line). Generation now
   runs 14 slots x 14 runs of 3,000 games with fresh engines (seeds 410000 + 100*slot + run):
   fresh engines measured 107 games/min per slot, batch 4's single long runs averaged ~50.
   Labels from game id 5,000,000; pool with `corpus_s37/pool_d8.jsonl` (~311M expected); s9 = r9
   trained **1 epoch** (q9 and r9 both kept epoch 1), seed 52; SPRT seed 3702
   (`matches/s37/sprt_s9.log`). The trainer now sizes its record array from a line count: past
   2^28 records the doubling step would briefly hold ~39 GB of arrays beside ~25 GB of SFENs.
   Training also picks up `2c04777` (two-digit hand counts such as 10P now parse; 111 of the first
   200k pool positions have one, so q9 and r9 trained on a sliver of misparsed hands).
7. Earlier handoff options (before the search measurements):
   - Search: measure node-efficiency to a verdict on more positions (YaneuraOu nodes vs ours to
     reach the same score band), then add per-rule toggles to `lean.rs` (LMR, LMP, futility, SEE
     pruning of quiet moves and drops) and find which one costs the most here.
   - Evaluation: positions like moves 48-49 (a major piece attacked by a backed lance or pawn,
     opponent holding drops) are where the net is far off. q9k (1.4% our-game positions) moved the
     collapse score 100-500 cp without changing moves; a larger share is still untested.


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

## Are depth-8 labels too shallow? (2026-10-26)

Every network so far learned from YaneuraOu depth-8 labels, and in the collapse positions YaneuraOu
at depth 8 is itself optimistic (q9 loss move 47: +405 at depth 8, +22 at depth 12, -493 in full).
`scripts/session37/relabel_depths.py` relabelled positions at depths 8, 12 and 16 (fresh engine per
200 positions, scores from the side to move); `label_bias.py` breaks the difference down
(`matches/s37/labels/`).

- **The user's four losses**, positions before our moves, contested stretch (|depth 16| < 1,500,
  move >= 30, n = 67): depth 8 is **+207** above depth 16 for us (median +129; 14 positions >= +300,
  none <= -300); depth 12 +70. Where we are well ahead, +52.
- **30k random positions from batch 5** (26k without mates): depth 8 vs 16 mean error **295 cp**
  (depth 12: 167), bias **+37** toward the side to move overall, +60 to +80 with 6+ pieces in
  hand, ~+50 from ply 40 on. On a 3k subset: depth 8 / 10 / 12 vs 16: |error| 300 / 234 / 168,
  bias +45 / +26 / 0, correlation (clamped) 0.968 / 0.980 / 0.991.
- Cost in game order (one worker): depth 8 2.5 ms per position, depth 12 32.5 ms (13x); depth 10
  ~2.6x depth 8 cold. Relabelling the 312M pool at depth 12 would take ~2 weeks.
- A depth-10 search's own depth-8 iteration is not a depth-8 label: in game order the table holds
  the previous move's depth-10 results (|difference| 240 cp from a separate depth-8 labelling), so
  the control below is labelled separately.

**Batch 6 tests label depth** (started 2026-10-26 07:03, `corpus_s37c/pipeline.sh`, log
`corpus_s37c/pipeline.out`, status `corpus_s37c/progress.sh`): chunked generation (seeds 420000 +
100*slot + run), labelled twice with the same game ids (from 6,000,000): depth 8 (`gen_d8`) and
depth 10 (`gen_d10`); two pools with `corpus_s37b/pool_d8.jsonl` that differ only in batch 6's
labels; t8 and t10 = s9 trained one epoch on each (seed 53); SPRTs t8 vs s9, t10 vs s9 and t10 vs
t8 (seeds 3703-3705, `matches/s37/sprt_*.log`).

Running notes: generation took 10 h, not 6.5: after a macOS restart with the user away, the
pipeline ran at 3-20 games/min per slot until `caffeinate -u` (`corpus_s37c/stay_active.sh`, renewed
hourly while the pipeline runs; keeps the display on) restored 107-114. Depth-8 labelling 4 h 20 min
(67.49M positions); depth-10 labelling runs at ~62 batches/h (~19 h, 4.3x depth 8). At 22:05 the
outer `pipeline.sh` was stopped (its depth-10 labeller kept running) and `corpus_s37c/pipeline2.sh`
took over, training t8 during the depth-10 labelling instead of after it (saves ~10 h).
Depth-10 labelling then stopped six times (05:40-07:30), each when one engine gave no move for
600 s: a single position (batch 583, `g_420714.csv` game 590, ply 111) on which YaneuraOu at depth
10 never returns (depth 8 was fine). `scripts/label_stream.py` now drops the rest of a game whose
engine stops answering and continues with a fresh engine; `corpus_s37c/label_d10_retry.sh` resumes
the labelling (13 workers, to ease memory beside the 47 GB trainer). Cost ~1.5 h.

**Batch 6 results (2026-10-28).** Pools: 374,835,085 (depth 8) and 374,834,947 (depth 10; the
dropped game tail). t8 trained 12.7 h (beside the labelling, under memory pressure), r_clamp 0.829;
t10 10.6 h, r_clamp 0.828 (its batch-6 held-out labels are depth 10). On the same held-out games
with depth-8 labels: s9 0.827 / 0.825, t8 0.828 / 0.826, t10 0.828 / 0.825 (old pool / batch 5).

| SPRT [0, 40], 1 s, 4 threads each | result |
|---|---|
| t8 vs s9 | **+27.9 ± 25.8**, 318-12-270, inconclusive at the 600-game cap (LLR +1.84) |
| t10 vs t8 | **FAIL**, -18.8 ± 44.5, 111-5-124 (LLR -3.07) |
| t10 vs s9 | 271-8-252 after 531 games (~+12), stopped by the user's request; LLR fell from +2.5 to -1.5 |

**Depth-10 labels did not make a stronger network** than depth-8 labels on the same positions, at
~3x the labelling cost, although they halve the side-to-move bias and cut the error vs depth 16 by a
fifth. Batch 6 itself adds a small gain (~+25-30), smaller than batch 5's +80; not shipped (no
SPRT pass). The SPRTs from 03:17 ran two at a time (`corpus_s37c/pipeline3.sh`, `pipeline4.sh`):
one engine per match thinks at a time, so two 4-thread matches fit the 16 cores.

## The user's s9 loss (2026-10-25, two-piece handicap)

`matches/user_games/2026-10-25_2mai_loss151.txt` (+ `_analysis.jsonl`, ours 1 s / YaneuraOu 1M):
opponent most likely the halfkpvm YaneuraOu, not Suisho5 (`scripts/session37/which_engine.py`: on the
19 White moves where the two disagree, the game followed halfkpvm 8, Suisho5 5 at 4M nodes; the
control game `2026-10-24_2mai_win140`, played against Suisho5, comes out 5 / 17). The same shape as
the two Session 36 losses: YaneuraOu +2,500 early, a slow leak to +1,500 by move 48 with no single
mistake over 150 cp, then 1g1f (move 49, -317) and 7b7a+ (54, -793), and the turn at moves 55-57,
which YaneuraOu sees 2-3 moves before we do (move 56: ours +382, YaneuraOu -198; 57 7f8e: +140 vs
-857; 58: -227 vs -2,000; the same with a cold table). At move 57 our search finds YaneuraOu's
2e1c+ only at depth 15 (10 s, 4 threads) or 18 (60 s), and still scores it -250 to -280 against
YaneuraOu's -875: depth per node and evaluation, as in the Session 37 search check.

## The user's second s9 loss (2026-10-26, two-piece handicap, vs YaneuraOu halfkpvm)

`matches/user_games/2026-10-26_2mai_loss143.txt` (+ `_analysis.jsonl`). The opponent is known: the
GUI's engine "YaneuraOu NNUE V9.00Git 64APPLEM1 TOURNAMENT" is the halfkpvm build (the Suisho5 copy
reports the same USI id but is a separate GUI entry, "YaneuraOu-Suisho5"); default options, 4
threads. On this known game `which_engine.py` leans the right way only weakly (7 / 5 at 4M nodes,
6 / 6 at 1M): the GUI opponent searches with 4 threads on a clock, so asking the user is better.

A different shape from the earlier losses: a few real mistakes early (moves 11, 20, 22: -345, -293,
-573), 4d4e at move 34 (-865; YaneuraOu +317 after it, ours +1,222), then **20 moves of steady
optimism**, 800-1,300 cp above YaneuraOu (move 45: +647 vs -214; 55: -158 vs -1,505), and 6g7f at
move 56 (-2,301). Re-searched (4 threads): at move 34 we now pick YaneuraOu's 4d4c at 1/10/60 s (the
game move was search noise), still scoring +1,050 to +1,310 against YaneuraOu's +832 (cold). At
move 56 we pick YaneuraOu's P*7d at 1 s but switch to the losing 6g7f at 10 and 60 s, scoring it
-321 to -443 against -1,720: more search makes it worse, so this one is the evaluation.
