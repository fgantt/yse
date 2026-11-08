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
