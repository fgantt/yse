# Session 40 research: where the gap to YaneuraOu is, and what to do next

**Written**: 2026-11-05. **Status**: proposal, awaiting the user's choice. Nothing multi-day has
started. The h3 side job (a1's data share, `HANDOFF_S40.md` §3) is training on the PC.

## 0. Summary

- **Our data, teacher and network-size levers stopped paying at the same time** (c1/c2,
  c1s, c1m/c2m, h2). The opponent that beats the user's games is **YaneuraOu V9 search with a
  network in our size class** (the PC's build runs Suisho5, HalfKP 256x2-32-32). Session 35 had
  already put the evaluation on equal terms and still found our tree needed ~5x the nodes.
- **The field's largest recent gains were in search, not networks.** YaneuraOu's 2025 search
  overhaul gave **R50 at 1 s/move, R90 at 4-8 s**, and "R90-120" according to several developers.
  The author credits **SPSA retuning** for shogi (chess-tuned constants were wrong) and
  **correction history**. Single steps of that work measured +20 to +40 each. We have neither.
  Our pruning margins are hand-set.
- **Changes to network shape are worth tens of R, and mostly cost speed.** In tanuki-'s 5,000-game
  tests, HalfKP 512 vs 1024 vs 1536 wide and 8-64/8-96/8-128 tails differ by +12 to -80 R. The
  sign is set by nps as often as by loss. Suisho10, the WCSC35 winner, is still **HalfKP**
  512x2-8-64. The one big evaluation claim is YaneuraOu's author's "+R100" for SFNN
  (HalfKA_hm2 1536, layer stacks). That is a large rebuild, and its speed cost is unknown for us.
- **Strong shogi networks train on ~8 billion positions** (tanuki-: "10億局面 × 8 セット", depth 9,
  ≤50k nodes, from Floodgate openings), often with distillation from deep-learning teachers. We
  have 1.14-2.06B. h2's extra 1B gave +21 ± 25. More data is still a lever, but a costly one.
- **Our lost games show no blunder to fix.** The judge drops below -300 between ply 35 and 67,
  and the damage is spread over many 40-150 cp moves. On those moves YaneuraOu's own choices
  swing just as much under a 3M-node judge, so move-by-move classification is below the noise
  (§1). The gap has to be measured in aggregate (§4, step A).

- **Step A done (§4 A results): we never reach parity at fixed nodes.** Against YaneuraOu +
  Suisho5 at 100k nodes we score 3.5% at 1x and ~20% at 8x and 16x (about −240), with our tree
  as narrow as theirs. The gap is per node, not in node count, so speed is not the lever.
- **A2 splits it: about half network, half search.** Against YaneuraOu with halfkpvm (our
  teacher) we score 10% at 1x and 42% at 16x (−56). Suisho5's network is worth ~110-200 to
  YaneuraOu. Against either network, 1x→4x gains little and 4x→16x a lot, which points to a
  shallow-search defect worth checking first (self-play node scaling, ~1 h).

**Original recommendation (before A):** first measure the gap in aggregate, cheaply: a node-equivalence match and a
tree-shape recheck (step A, ~3 h of Mac time in a window the user picks). Then take on search,
starting with correction history and an SPSA harness. Network width and data come after,
unless step A shows the gap at equal nodes is mostly evaluation.

## 1. The user's lost games (new tool: `scripts/game_gap.py`)

Five even losses (g5, h4, h4 vs Gikou 2, m8, a1; all as Black). Phase 1 scores **every** one of
our moves with the default judge (YaneuraOu halfkpvm, 1 thread, 3M nodes; the older
`*_analysis.jsonl` files left about half the moves unscored). Phase 2 takes the moves that lost
≥40 cp while the game was open (|judge| < 1500). Our current engine (a1, 1 thread) and YaneuraOu
each choose a move at 25k, 100k, 400k, 1.6M and 6.4M nodes, and the judge scores the choices.
Rows: `matches/user_games/*_gap.jsonl`, `*_ladder.jsonl`.

| Game (network) | judge < -300 from ply | open-phase loss: sum / mean cp | moves ≥40 / ≥100 cp | judge's best move |
|---|---|---|---|---|
| 10-29 vs YaneuraOu (g5) | 35 | 618 / 13 | 6 / 3 | 72% |
| 10-30 vs YaneuraOu (h4) | 63 | 1767 / 52 | 6 / 3 | 62% |
| 10-30 vs Gikou 2 (h4) | 45 | 1092 / 31 | 4 / 3 | 54% |
| 10-31 vs YaneuraOu (m8) | 61 | 461 / 10 | 3 / 1 | 52% |
| 11-01 vs YaneuraOu (a1) | 67 | 684 / 13 | 7 / 2 | 65% |
| 11-08 vs YaneuraOu (8934982, after this session's ship) | 53 | 1569 / 40 | 10 / 6 | 44% |

The 11-08 game was lost in a bishop manoeuvre over plies 43-53 (7g6h, 6h3e, 6f7g, 3e4f: judge
-47 to -524). Ladder at 1 thread: ply 43 found by us only at 6.4M (YaneuraOu 400k); plies 51,
63 and 77 our engine still plays the game move at 6.4M (an evaluation-side preference); plies
59 and 73 never found. One game, noisy judge: a pointer, not a verdict.

What the ladder shows (26 positions):
- **Our current engine avoids most of the older networks' costly moves** even at 25k nodes, at
  1 thread. In the g5/h4 games, a1 picks a different move in almost every one of those positions.
- **The judge is not precise enough to classify single moves.** YaneuraOu's own choices, scored by
  the same judge, lose 41-757 cp at some rung in 21 of the 26 positions, and they do not
  improve steadily with nodes. Rung results flip back and forth for both engines (e.g. 10-30
  ply 67: ours 0 / 52 / 0 / 1053 / 1053). Most of these "losses" are disagreements between
  near-equal moves at 3M nodes. This matches the earlier finding that equal judge scores are a
  measurement limit, not proof that two moves are equal.
- **Moves where YaneuraOu needs few nodes and we need many or never find it**, all late:
  a1 game ply 43 (we need 6.4M, YaneuraOu 400k), ply 65 (6.4M vs 25k), ply 97 (never vs
  1.6M); m8 game ply 59 (unstable for us, 25k for YaneuraOu). Too few and too noisy to
  divide the blame between evaluation and search.
- **Conclusion:** these losses come from accumulation, not blunders: many small edges over
  plies 30-70, then a position that falls apart. A per-move judge at our budget cannot assign the
  blame. Aggregate measurements can (§4 A).

## 2. Survey of the field

Sources at the end. "R" is Elo-like rating as each source reports it; self-play figures overstate
gains against other engines.

### 2a. Evaluation

| Finding | Value | Source |
|---|---|---|
| tanuki- HalfKP 1024x2-8-96 vs 1024x2-8-64 (random init, Hao data) | +11.9 ± 9.6; nps -2% | nodchip 2024-05-08 |
| 1024x2-8-128 vs 8-96 | -31.4; nps -7%; val loss slightly higher | 2024-05-13 |
| 1536x2-8-96 vs 1024x2-8-96 | -46.6 (random init), -79.8 (fine-tuned); val loss *lower*; nps -18% (L1 cache) | 2024-05-22 |
| 1024x2-8-96 vs 512x2-8-96 (distilled data) | +10.1 ± 9.6 at **65% of the nps** | 2025-08-07 |
| 512x2-16-96, 512x2-8-128 vs 512x2-8-96 | -17.4, -5.6 | 2025-07-20, 07-24 |
| HalfKA 1536-15-64 **layer stack** + cross-entropy, DL-relabelled | -27.7; "removing LayerStack might raise nps" | 2025-01-14 |
| HalfKP^ (factorised K and P) vs HalfKP, 256x2-32-32 | -44.4; "does not contribute" | 2022-06-20 |
| Suisho10 (WCSC35 winner) | HalfKP **512x2-8-64** | YaneuraOu eval list |
| Suisho11β | SFNN HalfKA_hm2 1536-15-32, 9 king-position layer stacks | same |
| YaneuraOu author: V9 search on Suisho10 | **+R120**; SFNN adds "about +R100" | V9 post, comments |
| SFNN gained threat features (piece attacked by an enemy piece), 2025-11 | no R given | YaneuraOu 2025 history |

Reading: lower validation loss often did **not** give more strength, because width costs nps.
That matches our c1m/c2m (middle layer, no measurable change). Leading NNUE engines either
stayed HalfKP and tuned the tail (Suisho10), or rebuilt everything as Stockfish-style SFNN with
large claimed gains. No source isolates SFNN's value from its data and teacher.

### 2b. Training data and recipe

- tanuki-: **8 billion positions** per data set ("10億局面 × 8 セット"), labelled at depth ≤9 and
  ≤50k nodes. Openings are Floodgate games with both sides rated ≥3900, randomised up to move 32.
  Qsearch "shuffle" is on (positions resolved by quiescence first, like our quiet filter).
  Duplicates are kept.
- Labels: in 2025 tanuki- used **distillation from deep-learning teachers** (DL Suisho,
  AobaZero ensemble). Lambda is 1.0 (pure score) in the distilled runs; earlier runs moved from
  1.0 to 0.5 late, and the 2025-01 run moved lambda to 0 by ply 60. Cross-entropy loss was
  tried.
- nodchip/shogi_hao_depth9 (MIT) is what we use: **1B positions per file group**, so our 400
  files are a fraction of it.
- Chess (Stockfish community): "at least a couple billion positions" for a 256-class network;
  depth-20 labels trained worse than depth-5. This matches our S37 finding that depth-10 labels
  were no better.
- **Us**: 1.14B (a1), 2.06B (h2, +21 ± 25 over a1, inconclusive). Our own quiet share fell from
  20% to 11%; h3 tests whether that mattered.

### 2c. Search

| Finding | Value | Source |
|---|---|---|
| YaneuraOu V9 overhaul vs before | R50 (1 s), R70 (2 s), R90 (4-8 s); "R90-120" | V9 post |
| Its two named causes | SPSA retuning for shogi (V9 only overtook the old version *after* SPSA); correction history ("contributes a lot") | same |
| Individual steps of that work (b1000) | step-16 reduction +42.8; IIR+ProbCut +40.6; shallow pruning +25.8; ProbCut +24.9; qsearch +14.9 / +22.4 | 2025 history (via summaries) |
| A pruning bug: correction term read as 0 in pruning conditions | pruned too much; found by measuring | 2025-08-01 entry |
| 4x cores | about +R100 | V9 comments |
| Stockfish: continuation histories ~63 Elo, main history ~11, fail-soft ~30 | no test conditions given | rentry sf_wisdom |
| Correction history (Caissa, 2023), adopted across chess engines; Stockfish 18 calls it central | — | CPW, SF 18 notes |
| SPSA on search constants, typical single run | ~+30 (chess, one report) | lichess blog |

**Us** (`src/search/lean.rs`, `lean_hist.rs`): fail-soft PVS, RFP, razoring, null move,
ProbCut, singular extension, history pruning, futility, SEE pruning, LMR. Histories: quiet,
two continuation plies, capture, killers, counter move. **Missing: correction history and any
automated tuning.** Every margin (`250*depth` razor, `100+150*depth` futility, `-4000*depth`
history, `-80*depth`/`-150*depth` SEE, the LMR table divisor 2.25) is hand-set or carried over
from chess values. Measured earlier: ~2.3x fewer nodes per second per thread than YaneuraOu
counting all nodes; S35 measured ~5x the nodes for equal depth at equal evaluation.

## 3. Where we stand, by part

| Part | YaneuraOu (PC opponent) | Us | Evidence of the gap |
|---|---|---|---|
| Network | Suisho5 HalfKP 256x2-32-32 | HalfKP 256→32→1, our own data | similar size class; ours trained on less, varied data |
| Search shape | SPSA-tuned, correction history | hand-tuned, none | S35: ~5x nodes at equal eval (before S35's history work) |
| Speed | — | ~2.3x slower per thread (all nodes) | 2x speed ≈ +50 R by the 4x-cores = +R100 rule |
| Data | (for its nets) billions | 1.1-2.1B | data gains now small (h2) |

## 4. Plan, ranked (needs approval)

Costs are Mac/PC time; "gate" is the usual 250 ms screens, then the 1 s SPRT vs stable.

**A. Measure the gap in aggregate (do first; ~3 h Mac in a window the user picks, plus offline
work).**
1. Node-equivalence: fixed-node games, ours at 1x, 2x, 4x and 8x YaneuraOu's nodes (e.g. Yane
   at 20k), both 1 thread, from the usual openings, about 100 games per ratio. Use the Suisho5
   build to match the PC opponent. Count all our nodes (`search_bench` all_nodes), or quote
   the reported ratio with the quiescence caveat. This gives "how many times our nodes we need".
2. Tree shape, offline (`search-diag`): per-ply node ratio and first-move cutoff rate now, vs the
   S34/S35 figures and YaneuraOu's ~2.3 per ply.
3. If (1) shows parity only at ≥4x nodes, the work is search and speed (B-D). If parity comes at
   ~1-2x, the gap is time use or evaluation (E-F).

**A results (2026-11-05).** Rule (3) did not anticipate this outcome: **we never reach parity, and
extra nodes stop helping.**

Node-equivalence (`matches/s40/node_eq.sh`; YaneuraOu V9 + Suisho5, 1 thread, `nodes 100000`,
no book; us = stable/a1, 1 thread, no book; 100 games per ratio, 50 openings × 2 colours):

| Our nodes (reported) | W-D-L | Score | Elo (95% CI) |
|---|---|---|---|
| 1x (100k) | 3-1-96 | 3.5% | −576 (−1200, −447) |
| 2x (200k) | 7-4-89 | 9.0% | −402 (−582, −307) |
| 4x (400k) | 7-0-93 | 7.0% | −449 (−676, −346) |
| 8x (800k) | 20-2-78 | 21.0% | −230 (−330, −156) |
| 16x (1.6M) | 18-4-78 | 20.0% | −241 (−344, −165) |

- Four doublings bought about +330, roughly 80 per doubling, and nothing from 8x to 16x
  (within noise). Our reported nodes leave out quiescence: 40-50% of the tree in the
  middlegames below (`search_bench` all_nodes in S3x put it nearer 75%), so 16x reported is
  about 30-60x YaneuraOu's total work. At that ratio we still lose by about 240.
- Tree shape (`search-diag`, start position, depth 4-10; `matches/s40/search_diag_d10.txt`):
  growth ~2.4x per ply (YaneuraOu ~2.3; S34 3.5-4x), 89.9% of cutoffs on the first move. The
  built-in "middlegame" position holds a forced mate in 13, which the mate search finds before
  any tree is built, so it reports zero nodes. Four middlegames from the user's losses instead
  (plies 41 and 61 of 2026-11-01 and 2026-10-30; `matches/s40/search_diag_mid_d10.txt`):
  2.0-2.4x per ply from depth 4 to 10 (one at 2.9), first-move cutoffs 83-90%, depth 10 in
  64k-1.2M reported nodes. **Tree width is not the problem.**
- **Reading.** The tree has YaneuraOu's width, and fifty times the work does not reach parity, so
  the missing strength is per node, not in node count. Each node is either evaluated worse
  (network) or searched with worse pruning, extensions and ordering. Speed (D) cannot close a gap
  that 16x nodes leaves open; B and C are worth tens of Elo each, not 240+.
- **Caveat.** The opponent's network is Suisho5, a stronger net than halfkpvm, which labels our
  data. The match does not separate "Suisho5's network" from "YaneuraOu's search". A2 below does.

**A2 results (2026-11-05):** the same match against YaneuraOu with **halfkpvm** (our teacher,
the same HalfKP 256 class) at 1x, 4x and 16x (`matches/s40/node_eq2.sh`, `neqh_*`). The seeds
match A, so each ratio uses the same openings:

| Our nodes | vs Suisho5 (A) | vs halfkpvm (A2) | Suisho5 network's margin |
|---|---|---|---|
| 1x | 3.5%, −576 | 10-0-90, 10.0%, −382 (−547, −290) | ~200 |
| 4x | 7.0%, −449 | 12-1-87, 12.5%, −338 (−477, −252) | ~110 |
| 16x | 20.0%, −241 | 41-2-57, 42.0%, −56 (−128, +12) | ~185 |

- **Both parts are large.** Swapping Suisho5's network for halfkpvm, with the same YaneuraOu
  search, is worth ~110-200 to it. Even against halfkpvm we need ~16x reported nodes (about 30-60x
  total work) to get near parity. So we have a network gap to Suisho5's network (data and
  training: F, G, E) *and* a per-node search gap to YaneuraOu's search (B, C, a search audit).
- **The curve has an odd shape, the same in both runs.** 1x to 4x gives little (−576 → −449,
  −382 → −338); 4x to 16x gives a lot (+210, +280). Gains per doubling normally shrink as nodes
  grow. A shallow-search defect would do this: something in our search that fails at small
  budgets (≤400k reported nodes, roughly depth 10-12) and stops mattering deeper. Each run
  alone is within noise (±100), but the two runs are independent and agree. Cheap check
  before B or C: ours vs ours at 1x/4x/16x (expected ~+80-120 per doubling), and `search-diag` by
  depth in middlegames (done: normal, see above).
- **Self-play check (2026-11-05, `matches/s40/self_scale.sh`, stopped early once decisive):**
  ours at 400k vs 100k nodes 23-1-2, at 1.6M vs 400k 16-0-2, i.e. several hundred Elo per 4x
  against ourselves. So our search does not stall at small budgets. The flat 1x-4x stretch
  against YaneuraOu belongs to the matchup (its search and network beat ours at those depths
  either way), not to a shallow-search defect of ours. The lever is per-node quality: B, C,
  and the network.

**B started (2026-11-05):** `src/search/lean_corr.rs`, correction history on the lean search's
static evaluation, keyed by pawn structure, pieces in hand and the opponent's last move,
`SHOGI_LEAN_CORR=1` (off by default; with it off the search is byte-identical to stable on the
test positions). At depth 9 on 30 corpus positions it cuts main nodes 39% and all nodes 26%, nps
−6%. Screens vs stable at 250 ms (`matches/s40/screen_corr_*.log`): s1 **+71.4 ± 54.3, PASS**
(87-4-57, 148 games); s2 +18.0 ± 27.6, INCONCLUSIVE at 600 (308-15-277). Pooled 748 games LLR
+2.18: probably +20-30, untuned. Ship SPRT at 1 s (`matches/s40/sprt_corr_ship1.log`): 127-12-131, −5.1 ± 36.3 over 270 games, FAIL (LLR −2.96). The 250 ms gain does not carry to 1 s with the untuned weights; not shipped. Kept behind `SHOGI_LEAN_CORR` (off by default) for SPSA to tune its weights and step, then retest at 1 s.

**C harness built (2026-11-05):** `src/search/lean_params.rs` makes 24 lean constants settable
(`SHOGI_LEAN_PARAMS=name=value,...`; defaults byte-identical with correction history off and
on; unknown names reported). `scripts/spsa.py` tunes them, plus the four correction-history
weights, by SPSA on `usi_match` opening pairs (1 thread, 100 ms, `MoveOverhead=20`; at the
default 100 ms overhead a 100 ms byoyomi leaves a 10 ms search and the engine shuffles into
repetition draws). First parameter set: `matches/s40/spsa_params_v1.json` (28 parameters).
Smoke-tested: ~6.5 s per pair per worker, so 20,000 pairs on 12-14 workers is ~3 h.

**SPSA run v1 (2026-11-06, Mac, 12 workers, 2.7 h, `matches/s40/spsa_v1/`):** 20,000 pairs, no
failures. The values moved little; every depth limit rounds back to its old value. Moved most:
hist_prune_mul 4000 → 3584, see_cap_mul 150 → 171, razor_margin 250 → 229, see_quiet_mul
80 → 86, rfp_margin 120 → 127; correction weights pawn 2 → 1.5, cont 2 → 2.7, step 8 → 7.0
(engine values 2,2,3,7). A small change, so expect a small gain at most. Screening
`matches/s40/screen_tuned.sh` (wrapper `tuned_v1`: these values plus correction on) vs stable,
4 threads, 250 ms, two seeds: s1 +85.7 ± 62.5 PASS (74-1-45), s2 +34.5 ± 27.7 PASS (317-10-259);
pooled 706 games LLR +5.69. Untuned correction history pooled only +2.18 over 748, so the
small moves seem to matter. Ship SPRT at 1 s: `matches/s40/sprt_tuned_ship1.log`.

**B. Correction history (1-2 days build, then gate).** Adjust static eval by a learned
per-structure correction (keys: pawn/hand structure, king neighbourhood, previous move). It
feeds into RFP, futility, razoring and LMR. Expected +15-40 by the chess and YaneuraOu
experience. Risk: YaneuraOu's own bug (the correction left out of the pruning conditions) shows
how easy it is to wire half of it. Test the fingerprint and pruning counts before games.

**C. SPSA harness for search constants (2-3 days build, then multi-day compute).** Expose the
~20 lean constants as USI options. Write an SPSA driver on `usi_match` (very short TC, 1
thread, many parallel games); the PC can run games as well. Expected +20-60. YaneuraOu's
experience says chess-derived constants are wrong for shogi, and we inherited ours. Our
cheapest large lever by the evidence, but the costliest in compute. Runs need the user's windows.

**D. Speed (profiling, open-ended).** Close part of the 2.3x per-thread gap (move generation,
accumulator updates, qsearch). Changes that leave `search_bench --fingerprint` byte-identical
cannot lose strength and stack. Expected roughly +25 per 1.5x.

**E. Network tail/width (1 week incl. trainer + engine + gate).** HalfKP 512x2-8-64/8-96, the
Suisho10 class. Expected +10-30 at best, and only if nps holds (tanuki- 1024 cost 35% of nps
for +10). Do after D: speed headroom pays for width.

**F. Data scale toward 4-8B (weeks of PC time).** More Hao files are the cheapest path.
Distillation from a deep-learning teacher is what the leaders do, but needs a licensed DL
network and GPU inference. Expected small per step (h2: +21 ± 25 for +1B).

**G. SFNN-style rebuild (HalfKA, threat features, layer stacks).** The largest claimed
evaluation gain (+R100 per YaneuraOu's author, unisolated), and the largest cost: new features,
incremental updates, trainer, data. Only after A says evaluation is the limit.

Proposed order: **A → B → C (harness, then runs in user-approved windows) → D**, re-measuring A
after each. h3 continues on the PC regardless.

## Sources

- nodchip (tanuki-) blog: [2024-05-08](https://nodchip.hatenablog.com/entry/2024/05/08/000000),
  [2024-05-13](https://nodchip.hatenablog.com/entry/2024/05/13/000000),
  [2024-05-22](https://nodchip.hatenablog.com/entry/2024/05/22/000000),
  [2024-03-20](https://nodchip.hatenablog.com/entry/2024/03/20/000000),
  [2025-01-14](https://nodchip.hatenablog.com/entry/2025/01/14/000000),
  [2025-07-20](https://nodchip.hatenablog.com/entry/2025/07/20/000000),
  [2025-07-24](https://nodchip.hatenablog.com/entry/2025/07/24/000000),
  [2025-08-07](https://nodchip.hatenablog.com/entry/2025/08/07/000000),
  [2022-06-20](https://nodchip.hatenablog.com/entry/2022/06/20/000000)
- [YaneuraOu V9.00 search overhaul](https://yaneuraou.yaneu.com/2025/08/20/yaneuraouv900-got-super-strong/);
  [update history 2025](https://github.com/yaneurao/YaneuraOu/wiki/%E3%82%84%E3%81%AD%E3%81%86%E3%82%89%E7%8E%8B%E3%81%AE%E6%9B%B4%E6%96%B0%E5%B1%A5%E6%AD%B42025)
  (some R figures read from summaries, not verbatim);
  [build notes (SFNN)](https://github.com/yaneurao/YaneuraOu/wiki/%E3%82%84%E3%81%AD%E3%81%86%E3%82%89%E7%8E%8B%E3%81%AE%E3%83%93%E3%83%AB%E3%83%89%E6%89%8B%E9%A0%86);
  [Suisho10 distribution](https://yaneuraou.yaneu.com/2025/05/15/about-the-distribution-of-suisho-10/)
- [nodchip/shogi_hao_depth9](https://huggingface.co/datasets/nodchip/shogi_hao_depth9/blob/main/README.md)
- [Stockfish search guide (rentry)](https://rentry.co/sf_wisdom);
  [Correction history (CPW)](https://chessprogramming.org/Static_Evaluation_Correction_History);
  [Stockfish 18](https://stockfishchess.org/blog/2026/stockfish-18/);
  [Stockfish NNUE docs](https://official-stockfish.github.io/docs/nnue-pytorch-wiki/docs/nnue.html)
- Data scale/depth in chess: [talkchess](https://talkchess.com/viewtopic.php?p=874538);
  SPSA: [weather-factory](https://github.com/lynx-chess/weather-factory),
  [lichess blog](https://lichess.org/@/kevlu8/blog/the-gruelling-road-to-2800-elo/7xyxsA3T)
- Ideas only: no code, constants or networks from these engines (memory
  `no_copying_from_yaneuraou`).

**Stopped (2026-11-06):** the 1 s SPRT of `tuned_v1` was stopped at 103-7-101 (LLR −1.31), after
reading the code turned up a game-only defect, then confirmed with a logging build. `get_best_move_with_limits`
(`src/lib.rs`) builds a new `IterativeDeepening`, and with it a new rayon pool, for every move. The lean
history and correction tables are `thread_local`. With 4 threads, the main thread kept its tables
but the three helpers were new threads with empty tables on each move (thread ids 3-5, then
17-19, then 30-32 over three moves). With 1 thread, the tables were created once. So every multi-threaded
game, including the ship gate and the GUI, ran three of four threads with cold history and
correction on each move, while the SPSA run (1 thread) did not. This may explain why correction history
gained at 250 ms and SPSA but not at 1 s/4 threads. Not yet measured. Nothing ever clears the tables
between games either.

**Correction to the above:** the default 4-thread search is Lazy SMP (`src/search/lazy_smp.rs`),
which spawns new helper threads for every search; the rayon pool is used only by the split.
**Fix:** each helper takes its lean history and correction tables from a store keyed by helper id when
it starts and leaves them there when it ends (`lean_hist::park/unpark`, the same in
`lean_corr`). All tables are rebuilt empty on their first use after `usinewgame` (`GAME_EPOCH`).
Checked with a logging build: tables are created once per thread per game, then again
after `usinewgame`. Single-threaded searches are unchanged apart from the per-game clear.
Gate: `matches/s40/sprt_fix.sh`, the fix with correction off vs stable, 1 s, 4 threads,
two seeds in parallel (s1, s2), to pool.
Result (2026-11-07): s1 229-8-229, +0.0 ± 31.7, FAIL; s2 175-8-133, +46.5 ± 36.8, PASS.
Pooled 404-16-362 over 782 games, about +19 ± 24, LLR −0.34: unresolved. The two seeds
disagree by more than their errors suggest (as with h3 vs h2), so the pooled figure is the
estimate: probably a small gain, which the [0, 40] gate cannot resolve.

**Tuned correction over the fix vs the fix (2026-11-07, 1 s, 4 threads):** s1 12-2-28, −139 ± 111,
FAIL after 42 games; s2 121-7-88, +53.5 ± 42.9, PASS. Pooled 133-9-116 over 258 games, about
+23 ± 37, LLR +0.29.

**The "seed 1 fails at once" pattern is not a harness fault.** Openings differ by seed. No file
in the repository changed during the matches, and the shared prefs file holds only
`thread_count: 4`. Over every S40 match, the first 40 games run both ways:
screen_corr_s1 24-15, screen_tuned_s1 27-12, sprt_tuned_ship1 14-25, sprt_tunedfix_s1 12-26. An
SPRT stops early only on an extreme start, so with two parallel seeds one is often an
early, extreme stop. Read the pooled figure. Stopping each seed on its own LLR wastes
the other's games and invites misreading. Pool with a single stopping rule instead.

**Estimate so far:** fix ≈ +19 ± 24 vs stable; tuned correction ≈ +23 ± 37 on top of the fix. Each is
too small for [0, 40] to resolve alone. Per the standing rule, gate the stack (`tuned_fix`
vs stable).

**Stack gate PASSED (2026-11-07, `matches/s40/gate_stack.sh stack1`):** the fix plus tuned_v1 values
plus correction on (`tuned_fix`), against stable, 1 s, 4 threads, two seeds under one pooled SPRT [0, 40].
s1 60-2-37, s2 52-3-39; pooled 112-5-76 over 193 games, LLR +2.97 PASS. The raw score is about +65.
An SPRT that stops at the boundary overstates the effect, and the parts' estimates (+19, +23) put
the true value nearer +40.

## 5. Rethink after the S40 gates (2026-11-08)

**What went wrong in this session's method.** After step A, the plan's order (B, then C) was kept
although A said the gap is per node and large. Changes worth about +20 were then put through a
gate that passes a true +20 about half the time. Screens ran at about 150 ms of search
(MoveOverhead 100) and SPSA at about 80 ms on one thread, while the gate runs 1 s on 4 threads;
search changes that won there were even at 1 s, twice. The 4-thread cold-table bug was in the
code all along and took minutes to confirm once looked for. It also means **every 4-thread
measurement of history-ordering changes since S35, including S35's own testbed, ran with three
of four threads' history reset each move.** Standing rule from now on (memory
`check_before_long_runs`): before any run of hours, write down the expected effect against what the
gate resolves, the state-lifetime/correctness check, whether the test conditions match the gate,
and whether the change addresses the measured gap, and state the odds of a pass.

**What the evidence says the gap is.** S35's testbed (`yo_nnue.rs`, YaneuraOu's network inside our
search, diagnostic only) held the evaluation equal and found our 4 threads x 500 ms (~1M nodes)
playing like YaneuraOu at ~7,000 nodes, and **~5x the nodes to reach depth 13** (median 580k vs
110k). S40's A/A2 agree: no parity even at 16x reported nodes. Evaluation also matters (42.5%
→ 95% at 3k nodes with their network), but the larger, measured gap is **how many nodes a depth
costs us at equal evaluation**. Everything this session shipped (+~40) is small against that.

**The levers, checked.**

| Lever | Size, by evidence | Can we measure it cheaply and in gate conditions? | Verdict |
|---|---|---|---|
| Audit 4-thread game path for more state/lifetime bugs (TT generation per move, helper accumulators, game history for repetition in helpers, eval caches, time use with 4 threads) | unknown; past finds +202 (hash) and today's | yes: code reading + logging builds, minutes each | **do first** |
| Re-run S35 testbed with the fix (our search + their network vs YaneuraOu, 10k nodes, 120 games) | resets the baseline for every search measurement | yes; large effects (S35 rows span 3-95%) resolve in 120 games | **do second** |
| Nodes-to-depth at equal evaluation (5x) | the largest measured gap | yes, offline: `search-diag` per-ply ratios and cutoff index vs YaneuraOu's own depth/node counts on fixed positions, minutes | **primary target**; each change also checked for accuracy (neutral-set move loss) before any games |
| D. Speed per thread (2.3x) | ~+50-80 by the doubling rule | yes: fingerprint-identical changes need no games at all; nps is measured in minutes | **run alongside**, stack, gate once |
| G. SFNN-style network rebuild | +R100 claimed (unisolated), weeks | only by games after weeks of work | after the search gap is down, or if the testbed says otherwise |
| E. Width/tail, F. more data | +10-30 and ~+20 per 1B | no: below the gate's resolution, days to weeks | **park** |
| More SPSA | small at short TC; tuning must run near gate conditions | no at current cost | **park** until search structure changes |

**Proposed order.** (1) Audit the 4-thread game path, offline. (2) Re-baseline the S35 testbed with
the shipped engine (one 120-game run at the 10k rung, ~1-2 h). (3) Work on nodes-to-depth at equal
evaluation using `search-diag` against YaneuraOu on fixed positions, with speed work (D) stacked
alongside; no games until a stack shows a large offline change. (4) Gate the stack once, pooled,
with the pass odds stated first.

### 5a. Audit of the 4-thread game path (2026-11-08)

Checked by reading the per-move path (`get_best_move_with_limits` → `IterativeDeepening::search`
→ `LazyHelpers`) and measuring with logging builds and the shipped engine (4 threads, 1 s byoyomi,
MoveOverhead 100, 40 positions from a gate game; `$SP/timeuse.py`).

| Item | Finding |
|---|---|
| TT aging | Sound: `new_search()` once per move, before the helpers start |
| Helper setup per move | Sound and cheap: network copies and engines take 3-7 ms on the main thread, 0.2-0.8 ms per helper |
| Helper game history (repetition) | Sound: helpers get the main engine's `game_path`, set every move |
| History/correction across moves | Fixed and shipped in `8934982` |
| **Partial iterations** | **Broken for the default search.** `partial_iter` (S32: +42.8 ± 35.0, SPRT pass) only applies when the root/PV *split* produced the result (`partial_usable` needs `parallel_result`). Lazy SMP became the default afterwards (`56ce0aa`), and since then a cut-off iteration is always discarded, on one thread or many |
| **Time actually used** | Moves end at a mean of 793 ms of the 1,000 ms byoyomi. The move played comes from the last *completed* iteration, which ends at a mean of **562 ms**. On average 231 ms (median 281, max 716) go to an iteration whose result is thrown away, and 24 of 40 moves waste more than 200 ms. The playing strength is that of a ~560 ms search |

**Consequence.** The engine plays as if it had about 56% of its byoyomi. Making a cut-off main-thread
iteration usable (previous best searched first; adopt the best root move that finished inside
the window, as `partial_iter` did for the split), and then letting iterations run to the limit,
would turn most of the ~340 ms per move into searched time: roughly 1.6x the effective time.
The S32 measurement of the same mechanism, +42.8 ± 35, is the best prior for its value. The
offline check, effective time at the last usable result from `timeuse.py`, takes a minute and
verifies the implementation before any games.

### 5b. Partial iterations for the main thread (2026-11-08, uncommitted)

`search_at_depth` records the root moves that finished (`last_root_completed`). When the clock
cuts an iteration of the main thread's own search (Lazy SMP or one thread), the iterative
deepening loop now keeps its best finished move if the previous best is among the finished moves
and the score is inside the window. These are the conditions `partial_iter` already used for the
split. The fixed-depth `search_bench` fingerprint is byte-identical to `8934982`.

Offline, with the same 40 positions, 4 threads, 1 s byoyomi and a logging build (`$SP/timeuse2.py`):

| Build | Iterations cut off | Partial adopted | Effective time (mean / median) |
|---|---|---|---|
| shipped `8934982` | (all discarded) | 0 | 562 ms / — |
| partial, start fraction 0.7 (default) | 16 | 8 | **680 / 688 ms** |
| partial, start fraction 0.85 | 34 | 15 | 685 / 740 ms |
| partial, start fraction 1.0 | 38 | 16 | 726 / 802 ms |

Expected value: +21% effective time (0.7), roughly +20-30 by self-play time scaling, plus the
better moves found late in an iteration. S32's +42.8 ± 35 for the same rule is the upper prior.
The start fraction stays 0.7: under a main-time clock a later start spends banked time, and at
byoyomi it adds only 46 ms more.

**Gate odds** (`scripts/sprt.py plan 0 40`, simulated): a true +20 passes 39% of the time (median 510
games), a true +40 passes 90% (median 306). My earlier statement that a +40 change passes only
about half the time was wrong; the 50% point is about +25.

**Gate result: FAIL.** Pooled two-seed SPRT [0, 40] at 1 s byoyomi, 4 threads each, partial vs
`8934982` (seeds 4071/4072): 179-13-180 after 372 games, about 0 Elo, LLR -3.12. The extra
~120 ms of effective search per move bought nothing measurable; not shipped. Time-to-strength
scaling at this budget is flatter than assumed, which also argues against more time-use work.
The 11-08 user game (§1) points at evaluation instead.

## 6. Plan: the network (2026-11-08, needs approval)

### Why the network, and a correction

- **The 11-08 user game** (§1): on the costly moves our engine keeps the game move at 6.4M nodes
  (plies 51, 63, 77) or never finds the judge's move (59, 73). More search does not change its
  mind; its evaluation prefers those moves.
- **A2** (§4): YaneuraOu's own search is worth ~110-200 more with Suisho5's network than with
  halfkpvm. A network of our size class carries a gap of that order.
- **S35** (§5): YaneuraOu's network inside our search took our score vs YaneuraOu-3k from 42.5% to
  95%. That was with the S26 network, which is much weaker than a1; the number is stale.
- **Correction.** I said after the partial-iterations gate that extra time is "worth little". The
  gate cannot show that: +21% effective time should be worth about +20, and the result,
  about 0 ± 30, does not rule that out. What it shows is that time use is not where large gains are.

### What we do not know, and the cheapest way to find out

Two questions decide which network work pays:
1. **Does a1 reach the quality of its own class?** halfkpvm is the same size (HalfKP 256x2-32-32)
   and labels our own data. If a1 is clearly worse inside the same search, our *training or
   features* lose strength and can be fixed cheaply. If a1 is equal, the next gain needs a
   *better teacher or more data*, which is costly.
2. **Does a better network fix the mistakes that lose games?** The 11-08 positions can answer
   that in minutes.

### Steps

**N1. Offline checks (Mac, under 1 h, no games, no window needed).**
- Static accuracy: a1 and halfkpvm (through `yo_nnue.rs`) on S35's balanced even-game
  positions (`corpus_s35/even/even_d14.jsonl`, |d14| < 1000), Pearson vs the depth-14 score at
  depth 1 and depth 8 of our search. S35 had s26 at 0.29 / 0.49 vs YaneuraOu depth 1 at 0.67.
- The 11-08 ladder positions (plies 43-77) with halfkpvm inside our search: does it avoid the
  game's moves where a1 does not?

**N2. Same search, two networks (Mac, ~1-2 h, a window you choose).** Our engine with a1 vs our
engine with halfkpvm (`SHOGI_YO_NNUE`), 1 thread each, fixed nodes (e.g. 200k reported), 120
games from the usual openings. Fixed nodes is the right test: the search is identical, and
`yo_nnue.rs` evaluates from scratch, so it would lose on time for reasons that don't concern
the question. Large differences (≥100) resolve in 120 games; if it lands within ±60, extend to 300
rather than read it. Same window, optional: the §5 testbed re-baseline (our search + halfkpvm
vs YaneuraOu + halfkpvm, 10k nodes) since the cold-table fix changed every search measurement.

**Decision after N2:**

| N2 result (a1 vs halfkpvm, same search) | Meaning | Next |
|---|---|---|
| a1 ≥80 below | our training/features lose strength at equal size and data class | **N3** (features) |
| within ±60 | a1 matches its class; the gap is the teacher/data | **N4** (teacher), costed first |
| a1 above | halfkpvm is not the target; Suisho5's margin is teacher/data | N4 |

**N3. Full king squares (HalfKP proper).** Our features condition on 9 king buckets; halfkpvm (45
mirrored squares), Suisho5 and Suisho10 all use the king's square. 81 buckets were tested only
with the old Rust trainer and small data: S24 (6.8M) −78; S35 (62.6M) k81 ≈ k9 (78% vs 80% at
3k, 27.5% vs 20% at 10k). They have never been tried with the GPU trainer and 2B positions,
which is the premise S24 left open. The GPU trainer already has the king factoriser; tables come
from `nnue_pack tables --king-buckets 81`.
- Prerequisites, each minutes and checked before training:
  1. **Share the network weights between threads.** Helpers now clone the weights every move
     (`clone_for_worker`, 10.8 MB x 3 helpers); at 81 buckets that is ~97 MB x 3 per move, and
     four copies also crowd the caches. Sharing by `Arc` keeps `search_bench --fingerprint`
     identical, so it ships with no games and helps today's network slightly.
  2. nps with an 81-bucket network (random weights do for speed): every king move refreshes the
     accumulator. If nps drops more than ~15%, add a refresh cache before training.
  3. Training time: a 1M-batch timing run on the PC; the estimate is 1.5-2x a1's 6.5 h.
- Train on `mix_h2` (h2 recipe), screen on the PC against a1/h2, gate on the Mac.
- **Odds, honestly:** no source isolates king resolution for shogi at our data size. The S35
  pair says it was neutral at 62.6M; the field's choice says it matters at billions. I'd put a
  pass of the [0, 40] gate at about 1 in 3. The cost is about a day of PC time, a few hours of
  my work and one gate.

**N4. Better teacher or more data.** Costed only if N2 points here. Candidates: relabelling a
large share (not c1s's 110M of 1.2B) with Suisho5, or more Hao files toward tanuki-'s 8B. Both
take days of PC time, and h2's +21 for +1B sets low expectations. Suisho5 labels were used
in S39, but check their license before using them at scale.

**Not in this plan:** width or tail changes (E: +10-30, below the gate), SFNN (G: weeks;
only if N3 works and the network stays the limit), more SPSA or time use.

**Before any gate in this plan** the four `check_before_long_runs` answers are written down, with
the pass odds.

### N1 results (2026-11-08)

Testbed build: `8934982` with `--features yo-diagnostic` (the `SHOGI_YO_NNUE` hook is compiled
out of normal builds; without the feature the variable is silently ignored). Start position
75 cp, as in S35. Scripts: `$SP/n1/acc.py`, `$SP/n1/ladder_yo.py`.

**Accuracy**: 3,000 balanced positions (|d14| < 1000) from S35's even-game set, Pearson vs
YaneuraOu depth 14. The judge runs halfkpvm, which flatters halfkpvm.

| | depth 1 | depth 8 | mean abs. error, d1 / d8 |
|---|---|---|---|
| YaneuraOu depth 1 (its own search) | 0.663 | | |
| our search + a1 | 0.682 | **0.841** | 200 / 162 cp |
| our search + halfkpvm | 0.726 | 0.827 | 186 / 149 cp |
| a1 − halfkpvm (paired bootstrap, 95%) | −0.042 [−0.065, −0.023] | +0.013 [−0.000, +0.029] | |

S35 had the S26 network at 0.293 / 0.491 on the same kind of positions: **a1 is now in
halfkpvm's class.** It is slightly worse statically and equal or slightly better after 8 plies,
even with a judge that shares halfkpvm.

**The 11-08 positions with halfkpvm in our search** (same ladder): it makes the same mistakes.
It plays 6f7g (ply 51), 1f1e (63), 6f6d (73) and 6d6f (77) at the deep rungs, as a1 does. It
avoids 7g6h at ply 43, where a1 needs 6.4M nodes, and is worse than a1 at 61 and 67. YaneuraOu's
own search with the same network found 59, 73 and 77 at 25k-400k nodes. That judge is the
same engine at 3M nodes, so that agreement is biased in its favour; but on this game, with the
network held equal, **the difference is our search, not the network.** My reading after the
ladder ("evaluation") was wrong: it did not separate the network from the search.

**Decision (by the §6 table):** a1 ≈ halfkpvm, so N3 (king squares, to close a gap to our own
class) is not indicated. N2 is unlikely to change that. The remaining levers are a better
teacher than halfkpvm (A2: Suisho5 is worth ~110-200 to YaneuraOu) and the per-node search gap,
which this game shows again.

### N4, costed: a stronger teacher (2026-11-08, needs approval)

**Who teaches a1 now.** a1's data is 80% Hao (nodchip `shogi_hao_depth9`, Hao network at depth
9) and 20% our own games labelled by halfkpvm at depth 8. So "a better teacher than halfkpvm"
means, in practice, a better one than Hao. How Hao compares with Suisho5 is not measured
anywhere I found. A2 measured only Suisho5 vs halfkpvm inside YaneuraOu (Suisho5 ~110-200
stronger).

**Candidates (all published under MIT, which fits the licensed-data exception):**

| Set (Hugging Face, nodchip) | Teacher | Size | Notes |
|---|---|---|---|
| `shogi_suisho5_depth9` | Suisho5, depth 9 | 185 GB as 8 split 7z volumes, ~5B positions (YaneuraOu wiki), plus `shuffled.bin` 290 MB validation | shuffled; positions already replaced by the quiescence leaf (so already "quiet") |
| `tanuki-.nnue-pytorch-2024-07-30.1` | tanuki-'s 2024 network, depth 9 | 320 GB, 5 split 7z volumes | not shuffled, not quiet; teacher's strength unknown to us |
| `shogi_suisho5_depth9_entering_king` | Suisho5, entering-king openings | not checked | niche |

The Suisho5 network file's own licence is unclear (GPL v3 is listed for Suisho as a whole), so
relabelling with it at scale (as c1s did for 110M positions) is not the route. The published
data set avoids the question.

**Evidence for and against.** For: A2 (Suisho5 is much the stronger network in YaneuraOu), and
the field trains on billions of positions from strong teachers. Against: c1s, Suisho5 labels at
depth 8 on 9% of the mix (our own positions), gave no gain (−28, −14 vs c1). That is a weak test
(small share, our games' positions), but it is the only one.

**Steps.**
1. **Validation file only** (290 MB, minutes): `nnue_pack psv-check` for format and legal
   moves; label scale against outcome (the logistic scale, as for the S39 Suisho5 labels:
   560 vs halfkpvm 720, so `--eval-scale` ≈ 1.29 may apply); how it compares with Hao on the
   same measures. Stop here if the format or scale is wrong.
2. **Full download to the PC** (185 GB, ~1.5-2 h at the 30 MB/s of the Hao downloads). 7z keeps
   its index at the end, so a single volume cannot be extracted. C: has 291 GB free. 7z must be
   installed in WSL (`p7zip-full`).
3. **Extract ~0.92B positions** (37 GB) as a stream, pack with `pack-psv` (~35 min per billion),
   and mix with `ours_quiet` exactly as `mix_a1` (ours 222.6M + 913.6M Suisho5 in place of
   913.6M Hao). The PC's home disk has 108 GB free; the packed set (~92 GB) and the mix (~114 GB)
   need about 200 GB. **Deleting `mix_c1` (119 GB), `mix_a2` (65 GB) and `mix_h3` (216 GB) frees it;
   all are reproducible from packed sets. That needs your OK.**
4. **Train `p1_s5`** with a1's recipe and size (~5 h on the PC). A clean A/B: same size, same
   recipe, same own-game share; only Hao → Suisho5 changes.
5. **Screen on the PC** vs a1 (`pc_screen.sh`, two seeds). Mac gate only on a clear screen.

**Odds and what each outcome means.** If Suisho5's labels carry even a third of its network's
margin over halfkpvm-class play, that is +35-70, which the gate resolves (+40: 90% pass). c1s
argues it may carry little. I put the PC screen showing a clear gain at about 40%, and a ship at
about 30%. Cost if it fails: about a day of the PC, no Mac time. If it wins, the next steps are
(a) the h2-size version (Suisho5 + Hao + ours), and (b) the tanuki- 2024 set, tested the same way.

**Step 1 result (2026-11-08).** `shuffled.bin` (289,626,920 bytes, 7,240,673 records, fetched to
Mac `corpus_suisho5/val/`, ignored) decodes completely. Compared with a Hao file of the same size
(`thread_index=000`), first 400k records each, outcome turned to the side to move:

| | Suisho5 `shuffled.bin` | Hao `thread_index=000` |
|---|---|---|
| Stored move | none (`1a1a` in every record) | legal in all |
| In check | 22 (0.006%) | 54,706 (14%); 34% captures |
| \|score\| median / 90th pct | 626 / 3994 | 612 / 3688 |
| Ply median (10th-90th) | 78 (29-139) | 78 (29-140) |
| Win rate at +100-300 / +300-600 / +600-1000 / +1000-1500 | 0.576 / 0.669 / 0.767 / 0.849 | 0.573 / 0.673 / 0.778 / 0.854 |
| Best logistic scale (\|s\| < 1500) | 630 | 590 |

- Same generation pipeline (identical ply and score spread), already quiet. **Pack it without
  `--quiet`**: that filter needs the stored move and would drop every record. `pack-psv` on the
  file kept all 7.24M (9 s).
- **No `--eval-scale`**: the scales differ by 7% (630 vs 590), within what the S39 measurement
  of the same kind moved by. The S39 factor 1.29 came from Suisho5 run by us on our positions
  against halfkpvm, and does not apply to this set.
- The win-rate columns do not rank the teachers: these are different games, and both were
  played by the labelling engine itself. Only the trained network can tell them apart.
- Since nothing is filtered, the ~0.92B positions needed for the a1-shaped mix are ~18% of the
  set; 7z still needs every volume downloaded first.

## Council (2026-11-08, direction: the next lever while N4 trains)

Members: Claude Opus 5.5 (claudeAgent), GPT-6.1 Sol (codex), Gemini 4 Argon (cursor), all at
high effort, read-only, one shared brief. Muse Wonder 1.0 failed twice (plan mode not supported,
then API billing 402) and gave no view. Claims below marked *checked* were verified in the code
by the lead.

**Correction to §4 A/A2 (checked).** Since `849b44a` (2026-10-14) `lean_qsearch` feeds
`GLOBAL_SEARCH_NODES` (`src/search/statistics.rs:174`), so USI nodes and `go nodes` count
quiescence, as YaneuraOu does. `node_eq.sh` assumed otherwise. **16x reported was about 16x
YaneuraOu's work, not 30-60x.** The gap per unit of work is smaller than §4 A said; vs halfkpvm
we are at −56 with 16x the nodes. Also, under `go nodes` our cut-off last iteration is
discarded (§5b not shipped), so our effective budget there is below nominal.

**Adopted as candidates (all checked in code):**
- **The root search is the old `search_at_depth`, not the lean search.** Root moves after the
  first get full-depth null-window searches with no LMR or pruning
  (`search_engine.rs:6519-6545`), and are ordered by `order_moves_for_negamax` (`:6411`), not
  `lean_hist`. Raised by Claude and Gemini independently. It could raise nodes-to-depth (the S35
  ~5x) without changing the per-ply branching measured in §4 A.
- Helpers search every depth with a full window (`lazy_smp.rs:165`).
- Root TT entry is always stored as an upper bound, and the root is fail-hard: `best_score`
  starts at `alpha` and moves with it, so `best_score <= alpha` always holds at the store
  (`search_engine.rs:6351`, `:6649`, `:6789`).
- **SPSA v1 tuned five inert parameters** (`hist_prune_depth`, `hist_prune_mul`, `lmr_hist_div`,
  `probcut_depth`, `probcut_margin`): `SHOGI_LEAN_HIST` defaults to 1 and ProbCut to off
  (`lean_hist.rs:33`, `lean.rs:1009`). "hist_prune_mul moved most" (§4 C) was a random walk.
  Claude: measured in `c_end` steps, only `see_cap_mul` moved clearly beyond the dead ones, so
  "the small moves seem to matter" is not supported.
- Quiescence stand-pat uses the uncorrected evaluation (`lean.rs:1192`); interior nodes use the
  corrected one (`:353-363`).
- `QTABLE` (`lean.rs:1065`) is thread-local and not parked, so helpers start each move with an
  empty quiescence cache (lead's finding while checking GPT's claims; likely small).
- LMR excludes all checks, including losing ones that pruning already treats as quiet
  (`lean.rs:766` vs `:661`) (Gemini; small).

**Rejected or downgraded:**
- GPT: quiescence lacks repetition, and check chains bypass the 8-ply limit. Mostly does not hold:
  quiet checks are generated only at `qply == 0`, and after that the checking side plays only
  captures and promotions, which are irreversible. So quiescence terminates and cannot cycle
  internally. QTABLE reusing a with-checks entry for a no-checks probe is a documented design
  choice, not a proven bug.
- Gemini: "remaining depth 9 = 41-97 nodes shows root inflation of 2.5-4x". Those counts are just
  the number of root moves. The kill check should count all nodes under root moves by index
  (Claude's L1 check). Gemini's 65-95% pass odds for the root work are not supported; the lead
  puts a root-search stack at ~35-45%.
- Gemini: "the legacy orderer's history tables are empty all game". Plausible, not checked.

**Disagreements, open:** Claude thinks "speed is not the lever" (§0) is the belief most likely
wrong (§4 A's own table gives ~+80 per doubling); test: stable at 1.6 s vs 1.0 s, 4 threads,
~300 games, after N4 on the PC. GPT thinks "tree width is not the problem" is the weakest belief;
test: root-decision quality against total work at equal evaluation. N1's "the difference is our
search" leans on YaneuraOu agreeing with itself; Claude suggests a judge from another family.

**Next (lead's decision, offline first):** (1) logging build: all nodes under root move 1, 2-3,
4+ at depth 10-12 on the four §4 A middlegames (kill the root lever if 4+ is under 25%);
(2) `search_bench` profile for move generation and legality (Claude L2, fingerprint-identical
speed); (3) if (1) survives, one stack: lean root search (LMR, `lean_hist` ordering, fail-soft,
correct TT flag), helper aspiration windows, corrected stand-pat, parked QTABLE. Gate it once.
Switched-off features (singular extension, ProbCut, IIR, history LMR/pruning) wait for a
fixed-node screen calibrated on known results. Thread links: in the session transcript.

### Council follow-up: steps 1 and 2 (2026-11-08)

Logging build of `c57f5e4` (source identical to `8934982`) in a scratch worktree, 1 thread,
`nice`, no games. Counter: all nodes (interior + quiescence) under each root move, by move index.
Positions: the four §4 A middlegames (11-01 and 10-30, plies 41 and 61), depth 12.

**Step 1: share of all nodes under root moves 4 and later**

| Position | to depth 12, all iterations | depth-12 iteration only | root moves searched per pass |
|---|---|---|---|
| 11-01 ply 41 | 43.1% | 25.9% | 39 |
| 11-01 ply 61 | 84.4% | 87.4% | 76 |
| 10-30 ply 41 | 51.6% | 37.4% | 46 |
| 10-30 ply 61 | 70.6% | 69.0% | 80 |

The kill line (under 25%) is not met anywhere. **The root lever survives.** In the widest
position (11-01 ply 61) the 68 moves after the eighth cost ~4.6k nodes each, against ~30k for
the first move. They are searched at full depth, so a one-ply reduction would save about half of
each. Rough potential: 20-45% fewer nodes in these positions if the reductions lose nothing.
Lead's estimate: +15-40, about a third to pass the gate alone; stack it.

**Step 2: profile** (`search_bench --depth 9 --positions 150 --stride 400`, macOS `sample` for 20 s)
- **39% of the benchmark's search time is the root mate search** (`search_engine.rs:15677`). This
  is an artifact of the benchmark: the budget is `time_limit_ms × 10`, clamped to 5k-200k, and
  search_bench's unlimited clock gives 200k. At 1 s per move it is ~9k nodes, a few ms. **Exclude
  it when profiling with search_bench.**
- Excluding it: NNUE evaluation 27.5% (`forward_from_activations` 21%), legal move generation
  14.1% (legality test 8.3%), quiescence move generation 10.7%, SEE 10.9%, malloc/free/memmove
  5.6%, NNUE incremental updates 5.0%, make/unmake 4.3%, sorting 3.9%, hashing 2.1%.
- Main-search move generation plus ordering is ~18%, under Claude's 20% line. Lazy generation and
  legality checks alone would give ~10% speed (+5-10). **L2 as proposed is dropped.** Speed work,
  if any, is spread across evaluation, SEE and allocation, with no single large item.

### Council follow-up: the root stack, offline (2026-11-08)

Committed as `4dfe9c4` (built and tested on `c57f5e4` in a scratch worktree).
Six parts behind `SHOGI_ROOT_STACK` (bit set, default 0): 1 LMR for late quiet root moves
(move 4 on, depth ≥ 3, the interior table minus one, re-search at full depth if it beats
alpha), 2 `lean_hist` ordering at the root, 4 root TT flag against the original alpha,
8 aspiration windows in Lazy SMP helpers (±50, doubling, full after three), 16 correction history
on the quiescence stand-pat, 32 helpers keep their quiescence table across moves. With the
variable unset, `search_bench --depth 9` output is identical to the logging build of `8934982`.

**Nodes to depth** (`search-diag`, the four §4 A middlegames, `SHOGI_NO_MATE_SEARCH=1`), geometric
mean against config 0 at depth 10 / 11 / 12:

| Config | Ratio | depth 12 nodes, 4 positions |
|---|---|---|
| 0 (shipped) | 1.00 / 1.00 / 1.00 | 99k, 508k, 123k, 4.86M |
| 1 (root LMR) | 0.65 / 0.72 / 0.52 | 70k, 94k, 125k, 2.62M |
| 2 (root ordering) | 1.32 / 0.83 / 0.80 | |
| 3 (1+2) | 0.71 / 0.54 / 0.51 | |
| 16 (q correction) | 0.75 / 0.74 / 0.81 | |
| 23 (1+2+4+16) | 0.65 / 0.45 / 0.57 | 173k, 136k, 78k, 1.72M |

**Accuracy at fixed nodes** (`acc2.py`: 2,000 balanced positions of the S35 even-game set, `go
nodes 200000`, 1 thread, Pearson vs YaneuraOu depth 14, paired bootstrap 95%):

| Config | Pearson | MAE | mean depth | diff vs 0 |
|---|---|---|---|---|
| 0 | 0.8717 | 152.8 | 11.30 | |
| 1 | 0.8656 | 151.3 | 11.83 | −0.006 [−0.021, +0.006] |
| 3 | 0.8649 | 151.0 | 11.77 | −0.007 [−0.022, +0.006] |
| 23 | 0.8699 | 151.0 | 12.05 | −0.002 [−0.016, +0.010] |
| 0 at 400k (calibration) | 0.8809 | 146.8 | 12.59 | +0.009 [−0.004, +0.022] |

**The metric cannot resolve this.** Twice the nodes (+1.3 plies) is not significant on it either.
What it does show: no large loss of accuracy from the reductions. By MAE the stack recovers about
a third of a doubling; by depth at equal nodes (+0.75), about 60%.

**4 threads, 1 s byoyomi** (`tu4.py`: 40 consecutive positions of the 11-08 game in one process,
so state lives across moves): mean completed depth 13.97 (0) → 14.32 (23) → 14.60 (63, all
parts). No crash or hang; slowest move 965 ms. The gain shrinks with 4 threads, as expected when
helpers already cover root moves.

**Before a gate, the four answers.** (1) Effect: estimate +15-40 (≈ a third to a half of a
doubling); [0, 40] pass odds ~45%. (2) State: helper windows are local to `helper_search`;
QTABLE is parked per helper slot like `lean_hist`; root ordering uses each thread's own tables;
checked over 40 consecutive moves at 4 threads. (3) Conditions: the gate is 1 s, 4 threads, and
the 4-thread check ran in those conditions. (4) Gap: it addresses nodes-to-depth (S35 ~5x),
cutting it to 0.45-0.65x on these positions. Gate candidate: config 63 vs config 0, same
binary, pooled two seeds.

**Gate result: PASS.** Pooled two-seed SPRT [0, 40], 1 s byoyomi, 4 threads each, config 63 vs
config 0 of the same binary (seeds 4081/4082, `matches/s40/gate_root63_*`): **86-8-55 after 149
games, 60.4%, about +73 (±55), LLR +3.05.** All games ended by mate, move limit or repetition;
no crashes and no time losses. The offline estimate was +15-40 with ~45% pass odds, so the
offline metrics understated it. The nodes-to-depth cut (0.45-0.65x) was the better guide; the
fixed-node accuracy metric could not see even a doubling. Root LMR was the largest part offline;
the parts were not gated separately.

**Shipped (2026-11-09):** `4dfe9c4`, default `SHOGI_ROOT_STACK=63`. With 0 the `search_bench`
fingerprint is identical to `8934982`; all 264 `search::` unit tests pass either way. Installed
as `engines/usi-engine-stable` (previous: `-stable-prev` = `8934982`), `engines/usi-engine-4dfe9c4`
and `target/release/usi-engine`. `search_bench --depth 9`: all nodes 1.71M → 1.11M.
