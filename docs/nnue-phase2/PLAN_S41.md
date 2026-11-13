# Plan S41: the next levers for strength and speed

**Status**: r6 (steps 1-2 revised after runs 1-2, council rounds 1-3 on r6 applied), 2026-11-12; before that final (r5 + post-round-5 fixes), 2026-11-11, after council rounds 1 (Opus 7.0, Sol 7.6, Gemini 7.4), 2
(Opus 8.3, Sol 8.6, Gemini 8.5), 3 (Opus 8.8, Sol 8.9, Gemini 9.4) and 4 (Opus 9.0, Sol 9.0,
Gemini 9.7) and 5 (Opus 9.2, Sol 9.3, Gemini 9.8); Muse failed (billing). Nothing here has started.
Anything that takes hours of machine time needs the user's go-ahead. Changes are listed in §6.
**Shipped engine**: `f3fd207` (network a1; 2026-11-13, this plan's step 2a stack, gate 3 PASS), installed as `engines/usi-engine-stable`; before it `76fb942` and `6c611cf`.
**Sources**: `RESEARCH_S40.md` (cited by section), `HANDOFF_S41.md` §3, `SESSION_LOG_035.md`.

## 1. Facts that frame the plan

| # | Fact | Source |
|---|---|---|
| F1 | At equal evaluation (YaneuraOu's network inside our search, `yo-diagnostic` testbed), our search needed ~5x YaneuraOu's nodes to reach its depth 13 (median 580k vs 110k, 5 positions). Measured before `lean_hist`, the helper-table fix, correction history and the root stack. "Depth" is not the same unit in the two engines (different extensions and reductions; SESSION_LOG_035:383-385), so this ratio is descriptive only | RESEARCH_S40 §5 |
| F2 | Work-equivalence, YaneuraOu + halfkpvm at 100k nodes vs us (network a1) at 1x / 4x / 16x reported nodes: −382 / −338 / −56. Before the three search ships above | §4 A2 |
| F3 | Root stack `4dfe9c4`: nodes-to-depth 0.45-0.65x, gated **+73 ± 55** (offline estimate +15-40) | Council follow-up: the root stack |
| F4 | Known tree results that are still switchable: losing-check pruning in quiescence (`SHOGI_Q_PRUNE_LOSING_CHECKS`, `lean.rs:1250`): tree −27%, **+95.6 ± 67.7** PASS. Improving flag (`SHOGI_IMPROVING`, `lean.rs:177`): tree −24%, **−34.9 ± 53.5** FAIL. A smaller tree can play worse | SESSION_LOG_035 #6, #7 |
| F5 | **Neutral-set move loss is useless as a screen**: calibrated on F4's two results, two runs of one binary differed by 8 cp, and the worst engine scored best. Fixed-node Pearson accuracy cannot see 2x nodes either. **Fast games (250 ms, 4 threads, four parallel SPRTs) ranked both F4 results right, twice each** | SESSION_LOG_035:194-210; root stack accuracy table |
| F6 | But 250 ms screens gave false positives that vanished at 1 s twice (untuned correction history: screen +71 / +18, ship SPRT −5; one pruning change). After MoveOverhead 100 and the 50 ms safety margin (`search_engine.rs:16041`) a 250 ms screen searches ~100 ms | §4 B, §5 |
| F7 | Per-thread speed: ours ~470-480k nps counting every node, YaneuraOu 1.08M (1 thread): ~2.3x | SESSION_LOG_035 #2, #5 |
| F8 | Value of time: ~80 Elo per doubling vs YaneuraOu at fixed nodes (§4 A). Partial iterations (+21% effective time, worth ~+22 by that rate) gated 0 ± ~35, consistent with it (§6 "Correction"). Fingerprint-identical speedups cannot lose strength on their own and have shipped without games (rounds 2-4) | §4 A, §5b, §6 |
| F9 | Profile (search_bench d9, mate pre-search excluded, on `8934982`, before the df00d98 speed batch): NNUE eval 27.5%, legal movegen 14.1% (legality 8.3%), quiescence movegen 10.7%, SEE 10.9%, malloc 5.6%, NNUE incremental 5.0%, make/unmake 4.3%, sort 3.9% | Council follow-up: steps 1 and 2 |
| F10 | Search structure as it stands (checked in code): every non-losing check is extended one ply, bounded only by `current_depth` extensions per path (`lean.rs:779-782`); LMR reduces only quiet non-checking moves (`lean.rs:822-827`), so losing captures and promotions are never reduced (SEE pruning stops at depth 6); no shallow pruning of any move while in check (`lean.rs:720`); quiescence evasion pruning off by default (`SHOGI_Q_EVASION_PRUNING`, `lean.rs:1261`) | code |
| F11 | Switched-off features (singular extension ×3 settings, IIR, mate-in-one, ProbCut, history LMR) were screened once, on the S35 testbed, each at one setting, with three of four helpers' history reset every move (S40 fix), no root stack, IID unable to search (fixed 2026-11-10, off). None gained. History pruning was never screened alone | SESSION_LOG_035 "Screens on top of the history ordering"; §5 |
| F12 | SPSA v1 searched ~30 ms per move (byoyomi 100 − MoveOverhead 20 − 50 ms margin), 1 thread, 28 parameters, 20,000 pairs, 2.7 h on 12 workers; five parameters inert; only `see_cap_mul` moved clearly beyond them. Eleven parameters are integer depth or count limits | §4 C; `lean_params.rs:40-63` |
| F13 | Root stack at 4 threads: +0.35 ply completed depth vs +0.75 at 1 thread (fixed nodes) | root stack "4 threads" |
| F14 | Gate power, SPRT [0, 40]: true +20 passes ~39%, true +40 ~90% | §5b |
| F15 | Network levers measured out for now: a1 in our search ≈ halfkpvm in our search (N1); a stronger teacher gave ~0 (N4) | §6 N1, N4 |
| F16 | `go nodes N` (checked in code): runs on **one thread** (`lib.rs:582`); gets an unlimited time budget (`usi.rs:265-283`), so the root mate pre-search takes its maximum **200k df-pn nodes, not counted in N** (`search_engine.rs:15939-15948`; 22x its ~9k budget at 1 s); discards the iteration the limit cuts off, so completed depth is below what N suggests | code; RESEARCH_S40 Council 2026-11-08 |
| F17 | The testbed's YaneuraOu network is evaluated from scratch at every call: ~290k nps on one thread, about our own speed | SESSION_LOG_035:366-370 |
| F18 | Since A2 (F2), shipped search changes total roughly +110-150 in self-play (`8934982` ~+40, root stack +73, smaller ones). Parity at equal evaluation therefore probably lies near 6-10x, not below 2x | F2, HANDOFF_S41 §2 |

## 2. Standing measurement settings (every step)

- Every engine launch: a private `SHOGI_PREFS_DIR`; `Threads`, `USI_Hash`, `MoveOverhead` set
  explicitly; `usinewgame` per game; a copied binary with `resources/` symlinked.
- **Every fixed-node run** (steps 0a, 1, 2a): the root mate solver runs with a **fixed 9k-node
  budget** for both engines, as at the gate (~9k nodes at 1 s), not its 200k `go nodes` maximum
  (F16), and its nodes are reported apart from N. This needs a small diagnostic option
  (`SHOGI_MATE_NODES`; today the solver is only on or off, `search_engine.rs:15939`), checked to
  leave the default path byte-identical. Switching the solver off would bias the screen against
  check-extension candidates, whose extensions also find the mates the solver finds at the gate
  (Fable). Fingerprints (below) still run with the solver off.
- **Fingerprints** (step 3): `search_bench --depth 9 --positions 150 --nnue-weights nnue/default.bin
  --tt-mb 256` plus a1's feature flags, with `SHOGI_NO_MATE_SEARCH=1`, on a **prefiltered
  position file**: the 150 sampled records are written out first, positions that come out TIME-BOUND (in check with few replies get a 2-5 s clock,
  `search_bench.rs:146-148,187`) are dropped from it, and the identity runs read it with
  `--stride 1`, so no total includes them and no stride shifts. Compared, by a **whitelist of work fields only**: the `--fingerprint` file (index,
  move, score, main nodes; `search_bench.rs:232`), per-position `qnodes=` (`:196`) and the TOTAL
  line's node counts including `all_nodes` (`:238`). Every timing or rate field (`ms`, `nps`,
  `all_nps`) is excluded. A determinism check first (baseline twice, identical on the whitelist). Plus a **mate-solver fingerprint**: the solver alone at
  fixed nodes on the same positions and on the 469 mate-in-3 positions (S35), since it shares the
  move generator (`mate_search.rs:202`).
- **Seed blocks** (Fable): `usi_match` plays one game at a time from one seeded opening stream
  (`usi_match.rs:272,290-296`), so a run is parallel only across processes, one seed each (as
  `matches/s40/node_eq2.sh` and `matches/s41/gate_mac.sh` do). A run is therefore **a block of 12
  seeds, 12 processes**, each playing an equal share of the run's games. Where a rule compares
  "two seeds", it compares the block's two halves (seeds 0-5 vs 6-11). Blocks are assigned now:
  step 1 controls use 44100-44111 (root stack 100k), 44200-44211 (root stack 400k), 44300-44311
  (losing-check), 44400-44411 (improving), 44500-44511 (A/A); step 0a rungs use 43000-43011 (1x),
  43100 (2x), 43200 (4x), 43300 (8x), 43400 (16x), 43500 (32x) and 43600 (the a1 row), 12 seeds
  each; 2c' uses 43700-43711; step 2a runs take fresh blocks from 46000 upward in
  order; the stack confirmation's reserved block is **45100-45111** (none of these used before).
- **Screen estimates**: every screen run is a **fixed-length** pooled run over its seed block
  (paired random openings, colours swapped), reported as Elo and SE, never stopped at an SPRT boundary (a
  stopped SPRT overstates winners). `usi_match` prints ± **1.96 SE** (`usi_match.rs:356-360`), so
  SE = printed ± / 1.96; every rule below is in SE. At 600 games SE ≈ 14 Elo (pilot measures it).
- **Concurrency**: a 600-game run at 100k is ~4.5 core-hours; with a 12-seed block it runs 12
  games at once, ~25 min, most of the Mac. Fixed-node runs are load-immune but still slow the user's games; they run in
  windows the user picks.

## 3. Plan, in order

### Step 0. Measure (no clocked games)

**0b and 0c first** (~50 min, no games).
- **0b. Where our nodes go** (logging build, 20 min). On 40 game middlegames: main vs quiescence
  share; within quiescence by node type (stand-pat cut, capture, evasion, quiet check at qply 0);
  check extensions per path; nodes under moves made while in check; LMR-exempt moves late in the
  list (checks, losing captures, promotions). Descriptive: it orders step 2a's candidates.
- **0c. Profile, threads, pre-search** (30 min). macOS `sample` at 1 s / 4 threads on a 40-move
  game sequence. Mate pre-search elapsed time per move at 1 s. Time-to-depth at 1 vs 4 threads on
  the same sequence (F13): if 4 threads gain under ~1 ply, Lazy SMP joins step 2a's candidates
  (screened at 4 threads, below).

**0a. Work-equivalence at equal evaluation** (Mac ~2.5-3 h, fixed-node, load-immune). Testbed build of
`6c611cf` with `--features yo-diagnostic`, `SHOGI_YO_NNUE` = halfkpvm, checked to load
(`info string YaneuraOu network loaded`, `yo_nnue.rs:44`; start position ~75 cp). YaneuraOu V9 +
halfkpvm at `go nodes 100000`, 1 thread, no book; A2's openings and seeds (50 × 2 colours).
- **Pilot** (10 min, 20 games at 4x): loading, node counts, completion, time per game.
- **Rungs**, run until 50% is bracketed or the ends are reached: 4x first (100 games). If under
  50%: 8x, 16x (100 each), then 32x (60 games) if 16x is still under 50%. If 4x is at or above
  50%: 2x, then 1x. Costs from the pilot: among 4x/8x/16x, 16x is ~57% of our node work; 32x adds
  ~1 h.
- **One a1 row** (ours with a1, not halfkpvm) at the rung nearest 50%, to test N1 in games. N1
  showed similar judged accuracy, not equal strength (Astra). If a1's score differs from
  halfkpvm's at that rung by more than 2 SE, evaluation work reopens alongside step 2, without
  waiting for the search gap to close.
- **Read-out**: k, the node multiple at 50%, interpolated in log nodes between the bracketing
  rungs, with paired-opening uncertainty; if no rung brackets 50%, a bound against the last rung
  tested ("k > 32", "k < 1"). **k sizes and orders step 2; it is not a gate** (F18 makes small k
  implausible). Two results send the plan back to the council: a bracketed k ≤ 2, and k ≥ 16 (it
  would mean the +110-150 of self-play gains since A2 bought nothing against YaneuraOu, and that a
  self-play screen measures the wrong target; Fable).

### r6: steps 1-2 after runs 1-2 (2026-11-12; council rounds 1-3 applied; replaces the text below where they differ)

Why (§7, "Run 2"): the S40-41 ships measured +94 ± 18 in self-play at 400k nodes, +56 ± 26 at
1.6M, and +30 ± 29 against YaneuraOu at 1.6M; k ≈ 29x. The +94 → +56 fade is 38 ± 32 (1.2 SE):
suggestive, not established, but enough that a 100k screen (≈ depth 10-11) is the wrong budget.

**Depth at fixed nodes** (§7 "Depth vs nodes"; 0c's 40-move sequence, 1 thread, one process,
solver 9k): 400k 12.68, 600k 13.75, 800k 14.18, 1.2M 15.22, 1.6M 15.88. The gate (1 s, 4 threads)
completes 13.20 (0c), which 1 thread reaches at **≈ 510k nodes**. 1.6M is 2.7 ply past the gate,
800k about 1 ply past it. Lazy SMP's helpers also widen the tree, which depth does not show, so
the gate's work equivalent lies between ~510k and its 1.35M total.

**Measured costs** (12 processes, 1 thread each, 16 cores; file times in `matches/s41/`): 400k
self-play ≈ 2.6 core-min per game (600 games ≈ 2.1 h; 312 ≈ 1.1 h); 800k ≈ 5.2 (1,008 games ≈
7.3 h); 1.6M ≈ 10 (600 ≈ 8.3 h); one 240-game row vs YaneuraOu at 1.6M ≈ 1.5-2 h. SE at 400k ≈ 13
per 600 games (run 1's per-pair variance); at 1.6M, 26 × √(168/600) ≈ 14.

**Step 1 (calibration), ~4.3 h:**
- **Positive control: run 1** (production `6c611cf` vs `d09cd8e-a1`, 400k, its own block
  44600-44611, +94 ± 18, already done; Opus). It used the solver's 200k `go nodes` default, not
  9k; 0a found the two identical in score (37/60 identical games). The root-stack 23-vs-0 control
  and its 1,000-game and 400k rows are dropped.
- **Two new controls at 400k, 600 games each**, on the diagnostic build (`s41-diag`, default path
  checked identical to `6c611cf`; it alone has `SHOGI_MATE_NODES`) with the solver at 9k (Opus)
  (block 44300-44311 losing-check 1 vs 0 at an
  equal-time node budget: pruning off runs 8% faster in 0c, so the off side gets 432k nodes and the
  on side 400k (Gemini); 44400-44411 improving 7 vs 0). **Improving runs first**
  (Gemini): its 0c depth is +0.91 ply, so it is the control a depth-blind screen would get wrong;
  if its estimate is ≥ 0, step 1 stops there (2.1 h) and the fallback applies.
- **Acceptance**, fixed now: (a) the positive control (run 1) > 1.65 SE: met, 5.2 SE. Losing-check
  is judged by its sign only, through (d) and the stop rule: at equal time 0c measured it at only
  +0.24 ply, so it may be worth +15-30 today, not the old +95, and requiring 1.65 SE of it would
  fail a working screen ~60% of the time (Opus); (b) improving < 0; (c) seed agreement as one joint test over the **three** controls (run 1 and
  the two new runs, each on its own block; z from the block's halves), Σz² below the 95% point of
  **χ²(3) = 7.815**; A/A skipped if the pilot (20 games) shows determinism, else the A/A row
  within 2 SE of 0; (d) losing-check > 0 > improving. **Stop rule**: losing-check ≤ 0 → stop,
  fallback (Opus). Odds: F4's +95.6 and −34.9 came from stopped SPRTs, biased away from zero, so
  the controls are weaker than they look; a working screen still trips the improving stop ~19%
  (Opus). Passing (a)-(d) ~65-70%.

**Step 2a (screening):**
1. **Screen budget 400k** (≈ 0.5 ply below the gate). All three members adopted it over 800k
   (half the throughput for no shown gain in ranking).
2. **Two stages**: **312 games** (26 per seed, whole colour-swapped pairs; Sol), **stop if the
   estimate < −5** (Gemini: a true +15 is then stopped ~14%, not ~21% at < 0); otherwise the
   run goes on to **600** (50 per seed) and is judged on all 600: > +10 carries. Mechanism
   (Opus, Sol): `usi_match` cannot resume, but it writes and flushes its CSV after every game
   (`usi_match.rs:273,319`) from one seeded opening stream, so each run is launched with
   `--games 50` per seed, stage 1 is judged on exactly the first 26 rows of each seed's CSV once
   all 12 have them (`pool.py` gets a `--first 26` option; today it reads every row; Opus), and a
   stop kills the processes. Checked first on a few short games: the first
   26 rows of a 50-game run equal a 26-game run of the same seed. A stopped run is recorded as "stopped for budget", not as negative.
   The +10 threshold, the best-setting choice and stage 1 all bias carried estimates upward;
   no estimate from a screen run enters a stack sum or the odds (Sol, Opus, Gemini).
3. **Carries and confirmation**: the stack holds **at most two carries** (plus candidate 0,
   which is counted apart), with no per-carry confirmation; the stack confirmation on reserved
   seeds is the fresh-seed test (Opus). **The second carry ends screening** and the stack
   confirmation runs next. A third carry can never be funded inside the cap (more than two carries
   would each need a fresh-seed confirmation: reaching three takes ≥ 6.3 h of full runs, their
   confirmations 6.3 h more, plus 7.3 h for the stack: ≥ 19.9 h > 18 h;
   Gemini, Opus, Sol), so the plan has no three-carry branch.
4. **Order, by runs** (each a setting): 0 hand rule (only if the offline checks below keep it),
   1 check extensions "none", 1 the best other check-extension setting (cap 1 per path, or no
   extension for quiet drops, chosen by nodes-to-depth on the 40 positions), 2 singular extension
   with the best check setting (today's extensions if none carried; Opus), 3 LMR scope (late quiet checks, then late losing captures and
   promotions), 4 evasion pruning. The 40-position diagnostic (minutes) picks at most two settings
   per family before any games (Gemini).
5. **Early stop**: if the first three feature runs (candidate 0 not counted; Opus, Gemini, Sol)
   give no estimate > +10, step 2 stops and is reported (~5 h spent; Opus).
6. **Older rules below** (Opus): the equal-time rule and the 5% elapsed-time check stay, with
   outcomes inside this plan: `usi_match` first gets a mean think time per side (it reports only
   the longest today, `usi_match.rs:170`; minutes of work, before the losing-check control); for
   the controls and the screen runs the ratio is only recorded. **One rule sets the stack
   confirmation's node budget** (Opus; replaces the older 3%-nps rule below and any per-carry
   rescaling): before the reserved block starts, a short pilot of the final stack vs the baseline
   (24 games on non-reserved seeds, ~10 min, plus the 40-position nps check) measures the stack's
   elapsed-time ratio; if it is off by > 5%, the stack's node budget is rescaled by that ratio. The
   confirmation's own ratio is then recorded; if it is still off by > 5%, no gate runs and the
   result goes to the user (Sol). No candidate is sent to the clock. The 250 ms
   clock rescue for check-extension and singular items in (−10, +10] is dropped (clock games,
   outside the 18 h); 2c' (closing YaneuraOu rung) is replaced by the guard below; items 5-8
   (quiescence bounds, IIR/IID/ProbCut/history, mate-in-one on the clock, Lazy SMP) are outside
   this plan.

**Candidate 0: hand-rule scores in the main table (correctness).** 1.31% of main-table cutoffs use
a hand-repetition score (±30,000, `search_engine.rs:971,7284-7285`; path-dependent: the rule
compares against the path, including positions above the worker's root, `hand_path_prefix`,
`:7262-7266`; dependence is classified relative to the actual game root, Sol). The store
(`lean.rs:997-1012`) has no guard; `corr_learnable` (`lean.rs:1072-1075`) excludes them from
correction history only; `mate_score_to_tt` stores them unchanged (`search_engine.rs:67-75`), so
the range is 29,000 ≤ |s| < MATE_THRESHOLD. Facts from the round-1 reviews (checked in code):
`TranspositionFlag` has no "none" (`types/search.rs:26-30`) and flag bits 3 decode as `Exact`
(`thread_safe_table.rs:277-284`), so "store the move, no bound" is not available; a move-only
entry would also overwrite a path-independent bound at that key. Table scores are read in three
places: the cutoff (`lean.rs:354-370`), ProbCut (`lean.rs:496-500`) and singular extension
(`lean.rs:646-651`, which accepts |s| < MATE_THRESHOLD, so 30,000 passes). "Store but never cut"
(old setting b) is dropped: it leaks into singular extension and ProbCut.
- Settings: **(a) skip the store** when 29,000 ≤ |best| < MATE_THRESHOLD, at `lean.rs:1011`; the
  root store (`search_engine.rs:7040-7058`) is exempt (it supplies the root's table move and PV;
  Opus); **(c) skip the store when a hand repetition was scored anywhere in the node's subtree**,
  which also catches ordinary-valued parents of a ±30,000 child (Opus, Sol). (c) needs a
  per-search-thread counter (thread-local, like `Q_REPETITIONS`) with a snapshot at node entry,
  incremented **inside `hand_repetition_score`** (`search_engine.rs:7270`) whenever it returns a
  score, which covers all four call sites (main search `lean.rs:235-240`, quiescence
  `lean.rs:1387-1396`, ProbCut's quiescence entry `lean.rs:1305-1312`, and the older
  `search_engine.rs:7227` path), and counting **hand scores
  only**, not exact repetitions (`lean.rs:1394` returns either; counting draws would refuse almost
  every subtree; Opus). The same test also
  gates `corr_learnable` (`lean.rs:989`) so correction history does not learn a contaminated
  parent score (Sol, Gemini); helper threads and table reuse across moves checked in 2b. Optional
  narrower (c'): refuse only when the repetition matched a position above the node (Opus). **The
  offline checks pick one setting**; only it is played (Opus).
- **Offline checks first** (no games, ~30 min): (i) how many of the 3,443 cutoffs are
  branch-dependent (the matched `then` lies between the root and the node) rather than against
  the game path or inside the node's own subtree (Gemini); (ii) a sample of them re-searched fresh,
  without the table, at the entry's depth on the current path, **plus an equal sample of
  ordinary-valued cutoffs whose stored subtree met a hand repetition** (what (c) exists for;
  Opus): if ≥ 90% of both still hold, candidate 0 is dropped for this plan's budget (not "the bug
  is benign"; Sol); (iii) nodes to depth 12 on the 40
  positions: a setting adding > 5% nodes is rejected (Opus); (iv) setting (c)'s refusal rate.
- Regression tests, each failing without the fix: one main-search position under two histories
  (repetition against the path and not), both signs, an ordinary-valued ancestor, a genuine mate
  unaffected, a null-move floor, and table reuse across two searches (Sol).
- If it survives: a 312-game stage 1 only; below −1.5 SE dropped, otherwise into the stack
  without a carry threshold (prior −10 to +20, too small to show alone; Sol, Opus). It counts
  neither toward the early stop nor toward the two-carry limit. If it is the only survivor, no
  stack confirmation runs; it carries into the next stack (Opus).

**Stack confirmation**: **800k, 1,008 games (84 per seed, whole pairs; Sol, Opus), reserved block 45100-45111, ~7.3 h, SE ≈ 10.5:
estimate > +15 → 2b and the gate.** A null stack passes ~8%; a true +20 ~68%, +30 ~92% (Opus).
800k rather than 1.6M: closer to the gate's depth (above), and the same cost buys more games. **No
retry** (it would be another 7.3 h outside the cap; Sol, Gemini, Opus). Below +15 → no gate,
reported.

**YaneuraOu guard, after a gate PASS and before shipping** (Opus): the stack vs YaneuraOu +
halfkpvm (100k), ours a1 at 1.6M, solver 200k, seeds 43800-43811, 240 games, compared pair by pair
with run 2's `r2new_16x` row (the baseline `6c611cf`; reusable because step 3 changes only what
leaves fixed-node games identical: first 10 pairs replayed with the then-current baseline binary,
and the baseline row re-run if any differs). ~1.5-2 h. **Rule: stack − baseline < −29 (−1 SE) →
no ship, back to the council** (Gemini; a true −20 is caught ~38%, a true +25 falsely stopped ~3%).
It tracks a1-vs-YaneuraOu at 16x, not equal-evaluation k (Sol).

**Budget** (Sol: count runs, and fund every obligation before starting a run): step 2a's cap
stays **18 h including the 7.3 h stack confirmation**, leaving ~10.7 h: about 6 setting runs at
~1.7 h expected (2.1 h when not stopped). A run starts only if the remaining budget covers its
full 600 games and the stack confirmation; if not, screening stops and whatever has carried goes
to its confirmation (Sol). The second carry also ends screening (rule 3). **Total Mac**: step 1 ~4.3 h (2.1 h if improving stops it), step 2a ≤ 18
h, guard ~1.5-2 h if a gate passes: **at most ≈ 24.3 h** load-immune, plus minutes of offline
diagnostics and replay checks, in user-scheduled windows (Sol); the gate ~3-5 h pooled with the PC.

**Odds** (rounds 2-4, members' ranges; judgment, not measured power): step 1 passes ~65-70%;
reaching a gate ~25-55% including step 1 (Opus 25-35, Sol 35-50, Gemini 45-55); passing given a
gate ~30-45%; overall ~9-25% (Opus 9-13, Sol 11-23, Gemini 18-25).

### Step 1. Calibrate the screen (Mac ~3.5 h, fixed-node, load-immune)

The screen is **1-thread fixed-node games, as an elimination filter**: it removes candidates; it
does not estimate gate Elo (Sol, Opus). It cannot see Lazy SMP or per-node cost (F16, Gemini).
- **Controls**, each vs its off setting in the same `6c611cf` binary, 600 games each (fixed length,
  pooled over its seed block):
  root stack **23** vs 0 (the 1-thread parts; bits 8 and 32 act only in helpers, `lazy_smp.rs:152,
  173`; gate +73 at 63), losing-check pruning 1 vs 0 (+95), improving 7 vs 0 (−35), A/A.
  Budget 100k nodes. The root-stack control runs **1,000 games** (its 1-thread value at bits 23
  has never been measured; F3's +73 was at 63 with 4 threads; bit 16 also tests correction
  history on the stand-pat), and also 600 games at 400k.
- **Pilot** (20 games): records the per-pair variance (kept in the research doc next to the
  SE ≈ 14 assumption) and checks whether the paired games of one opening are identical (1 thread,
  pre-search and IID off, `usinewgame` resets state: `lib.rs:1606-1610`). If they are, the engine is
  deterministic here, the A/A control scores 0 by construction, and the 600-game A/A run is
  skipped.
- **Acceptance, fixed now, before any result**: (a) each positive control's estimate > 1.65 SE
  (one-sided); (b) improving's estimate < 0; (c) A/A within 2 SE of 0, or, if the pilot shows
  determinism, **one joint test** of seed agreement over the k = 4 control runs (root stack at 100k
  and at 400k, losing-check, improving; A/A is left out, as it is skipped): the sum over controls of
  z² (z = seed-1 minus seed-2 estimate over the SE of that difference) below the 95% point of
  χ²(number of controls), valid because every control run uses its **own disjoint seed pair**
  (its own seed block, assigned in advance, so no two control statistics share openings; z compares
  the block's two halves); (d) each positive control > A/A (or 0) > improving. The historical sizes
  need not reproduce, only (a)-(d). A working screen still fails (a)-(d) about **20-30%** of the
  time: ~20% if the controls keep their S35 values (root stack clears (a) ~87% at +30, (c) 95%,
  (b)+(d) ~96%), ~30% if the improving flag is nearer its pooled −28 ± 29 on today's engine (it
  and losing-check were measured on the S35 engine, before `lean_hist` and with the helper bug),
  where (b) holds ~85% (Fable).
- **Depth check** (root stack, 100k vs 400k; SE of the difference ~18): if 100k exceeds 400k by
  more than 2 SE, that is recorded (descriptive: depth-dependent carries get the 400k run anyway).
  If **400k exceeds 100k** by more than 2 SE, gains grow with depth, and every depth-dependent
  candidate (step 2a items 1, 2, 3, 6) that scored in (0, +10] at 100k is also run at 400k on fresh
  seeds before it is dropped; it carries only at > +10 there, the same bar as at 100k.
- **If (a)-(d) fail**: the screen is unvalidated, recorded as such, and not re-run at new settings
  until it passes. Fallback: the existing 250 ms 4-thread screen (right on F4, wrong on F6), with a
  1 s / 4-thread confirmation of every pass. The user is told before any further spending.
- **Frozen** once accepted: budget, openings, seeds, length and bounds do not change in step 2.
- No reproducible "false positive" control exists (F6's untuned correction-history weights were on
  an older binary); the 400k and 1 s confirmations stand in for it.

### Step 2. Tree efficiency at equal evaluation (the primary lever)

**2a. Candidates**, ordered by 0b, ≤ 3 settings each (F11; Sol: features need their margins):
1. **Check extensions** (F10): cap at 1-2 per path; no extension for quiet checking drops; none
   (`SHOGI_NO_CHECKEXT`). Prior: Stockfish removed its unconditional check extension and relies on
   singular extension; YaneuraOu's current rule to be read (ideas only).
2. **Singular extension together with (1)**.
3. **LMR beyond quiet moves** (F10): quiet checks; losing captures and promotions late in the list.
   Trap (Gemini): `quiet_score` (`lean_hist.rs:202`) is never updated for captures or promotions,
   so their reduction must not read it (`lean.rs:847-849`).
4. **Evasion pruning** (F10): main search in check once a non-mated score is found; quiescence
   `SHOGI_Q_EVASION_PRUNING`.
5. **Quiescence bounds**: beyond a few plies, recaptures on the last square only. **Cache
   context** (Astra): the quiescence table's `QSlot` (`lean.rs:1137`) records key, score, bound,
   checks and plies left, not the move set searched, and the probe (`lean.rs:1407`) cannot tell a
   restricted search from a full one. So the slot records the restriction (mode and recapture
   square) and a probe uses only a compatible entry, or results the restriction affected (and
   their ancestors) are not stored. Evasions in check stay complete. Regression test: one position
   reached through different last-move squares, and a restricted result then probed by an
   unrestricted search; it must fail without the safeguard.
6. IIR, IID (fixed, off), ProbCut, history LMR and history pruning.
7. **On the clock, not fixed nodes**: mate-in-one at nodes (`lean.rs:614`; it runs after the node
   is counted, `lean.rs:332`, but its move generation and tests count as no nodes).
   A pruning change can move nps without changing what a node costs (it shifts the mix of main and
   quiescence nodes), so nps alone does not send a candidate to the clock. Instead every fixed-node
   candidate's nps is measured before screening (1 thread, the 40-position sequence; the F4
   switches too, in 0c), and a candidate whose nps differs by > 3% is screened at **equal time**:
   N × nps_cand / nps_base nodes, still load-immune. Its nps ratio is logged with its estimate.
   That is an estimate of equal time, not a guarantee (Astra): every screen run, at 100k and 400k,
   logs both engines' mean elapsed time per move from interleaved games (`usi_match` keeps only
   the longest, `usi_match.rs:170`, so it also reports the mean, next to the time-loss rule), and a survivor whose
   elapsed-time ratio is off by > 5% goes through the clock confirmation of items 7-8 as well. The
   same policy applies to the step 1 controls (losing-check pruning likely moves nps).
8. **At 4 threads**: any Lazy SMP candidate from 0c.

   Items 7-8 use the 250 ms 4-thread screen, fixed length 600 games pooled over a seed block: > +10
   → a 1 s / 4-thread confirmation, 600 games on fresh seeds: > 0 → stacked; otherwise dropped.

- **Disposition** (600 games at 100k): estimate > +10 → carried. Estimate ≤ +10 → dropped for this
  plan's budget (recorded as inconclusive if > 0). Settings of one knob are alternatives: only the
  best setting above +10 carries. Two settings are combined only if they are orthogonal sub-knobs
  (e.g. a path cap and no extension for quiet drops), and the combination is screened itself.
  Nodes-to-depth is a diagnostic only.
- **False carries**: with SE ≈ 14, a candidate with no effect clears +10 about 24% of the time, so
  expected false carries ≈ 0.24 × (number of null candidates), about 3-4 if most of 12-18 runs are
  null. The pilot's measured SE replaces the 14. The confirmations below exist for this reason.
- **Check-extension and singular items (1, 2)** that score in (−10, +10] at 100k go to the 250 ms
  clock screen (items 7-8 rules) before they are dropped: they interact with mate finding, which a
  fixed-node screen models only through the solver's budget (Fable).
- **400k confirmation**, on **fresh seeds**, for every depth-dependent carry (1, 2, 3, 6), 600
  games: estimate > 0 → stacked, else dropped. The other fixed-node carries (4, 5) get a fresh-seed
  run at 100k (600 games, > 0), so no best-of-three estimate enters the stack unconfirmed.
- **Before the stack confirmation**, the stack's nps is re-measured on the 40-position sequence
  (several changes under 3% each can add up); above 3%, the stack runs at equal time as above.
- **Stack confirmation** covers the fixed-node carries (items 1-6) only. Clock-confirmed items 7-8
  (already confirmed at 1 s / 4 threads; a 1-thread fixed-node run cannot see them) join the stack
  at 2b and 2c.
- **Stack confirmation** on the **reserved seed block 45100-45111** (frozen now, used by no
  earlier run), 1,000 games at 400k: estimate > +15 (≈ 1.4 SE) → gate. The ~9% chance that a
  null stack passes applies to this **first, prespecified attempt only**. If below, the weakest
  member is removed (ranked by its latest fixed-node estimate: 400k where it has one, else 100k;
  clock-screened members, items 7-8, are not in the removal pool, and with no fixed-node member
  the retry is skipped) and the stack re-run once on the same reserved
  block; that retry reuses revealed data, so it is **exploratory**: a pass there still goes to the
  gate (which is independent), but its odds are reported as lower. Below again → no gate, report
  to the user.
- **Budget**: ~12-18 screen runs at 100k (~25 min each), ~3-5 confirmations at 400k (~1.5 h
  each), two fresh-seed 100k runs (~25 min each), one or two stack runs (~2.5 h each), and any
  extra 400k runs the depth check triggers (~1.5 h each): **~13-21 h of Mac (middle ~17 h), cap
  18 h, which will probably bind**; step 0b's ordering decides what is cut. In chunks the user
  schedules. Clocked items 7-8 add, per candidate, a
  250 ms screen (~1.5 h) and, if it carries, a 1 s / 4-thread confirmation (~3 h pooled Mac + PC,
  clock-sensitive: user-free windows only); expected 1-2 such candidates. **At the cap**: no new candidates (the next in 0b order is where it stopped); what has
  carried is confirmed and stacked, and the rest is reported.

**2b. Before the gate**: the 4-thread 40-move sequence check (`tu4.py`, an S40 scratch script, rebuilt
if lost; RESEARCH_S40 root stack "4 threads") for crashes, time losses and state across moves;
new state parked with the helpers and reset on `usinewgame`; unit tests for each new rule, each
checked to fail without its change.

**2c'. Closing measurement against YaneuraOu** (Fable), alongside 2c: the confirmed stack replays
step 0a's bracketing rung (~400 games, fixed nodes, testbed with halfkpvm, ~1-1.5 h). Every
decision from step 1 on is self-play; this checks that the stack moved k too (SESSION_LOG_035
item 8 records self-play gains that did not carry over). Reported with the gate; it does not
decide shipping alone, but a stack that passes the gate and leaves k unchanged goes to the council
before more of this kind of work.

**2c. Gate**: one pooled two-seed SPRT [0, 40], 1 s, 4 threads, 256 MB, Mac and PC, stack vs the
then-current baseline (which already includes step 3's speed work). **Timing rule** (Astra):
`usi_match` measures think time but only warns past 2,000 ms (`usi_match.rs:168-176`) and never
adjudicates. Before the gate it gets a time-loss rule: a move beyond byoyomi plus a stated latency
allowance (measured with a mock engine that sleeps a set time; seconds, no real games) loses the
game, and each log's longest think is checked before its SPRT result is accepted. The four answers and pass odds
in writing first. If it FAILs, one revision and one more gate at most; no third. **Odds**: ~30-40%
for the first gate, *given* a stack reaches it (council 25-50; Fable ~30). Reaching the gate at
all: ~60-65%. Given the confirmation: estimate +15-25 → ~30%, ≥ +35 → ~65% (Fable), read as an
upper bound (1 thread, fixed nodes). A stack confirmed below ~+25 may wait for SPSA v2 to stack
on it rather than gate alone (the user decides).

### Step 3. Speed, byte-identical (alongside step 2; ships into the baseline before 2c)

Targets from 0c. Candidates: NNUE forward (the int8 dot-product path on NEON), lazy legality with
identical move lists and order, SEE computed once per move instead of in ordering and in pruning
(`lean_hist.rs:241,256`, `lean.rs:709-715`), SEE threshold tests where only a comparison is used,
allocations in move lists, quiescence's double sort (`moves.rs:1132` then `lean.rs:1467`: fold
the generator's comparator into the second key as a tie-break; removing it alone changes ties,
RESEARCH_S40 rounds 2-4).
- **Ships without a strength gate** when: both fingerprints (§2) are identical; `move_legality_tests`
  and the move-generator oracle tests pass; differential tests for NNUE arithmetic and SEE against
  the old code on 30k positions pass, on the Mac (NEON) and the PC (AVX2); interleaved 1-thread
  fixed-depth timing shows the gain; the 4-thread sequence check shows no crash or time loss and
  no nps loss. This is equivalence on tested coverage, not proof; Lazy SMP scheduling still shifts.
- Shipping into the baseline first keeps a speed gain from masking a weak tree stack in 2c, and
  from being held back if 2c fails (Gemini, Opus). Expected +10-30 at 1.2-1.3x.

### Step 4. SPSA v2 (after step 2 changes the tree; its own approval)

- **Pilot first** (100 pairs, ~10-15 min, all 12 workers): throughput at the chosen conditions
  (e.g. 1 thread, byoyomi 200, MoveOverhead 10, ~140 ms search); the full run's pair count and cost
  are stated from it. (r1's "2 threads, 300 ms, 6-10 h" was wrong: ~38 h.)
- Continuous margins and divisors only; correction history on (`spsa.py --corr`, `spsa.py:90`). The
  correction weights are small integers (2, 2, 3, 7), so the pilot also checks whether they need
  finer units (an engine change) before they are tuned.
- **Drift reference**: each parameter's movement normalised by its own step (`a_p`, `c_p`,
  `spsa.py:69`) and compared with the null drift expected under no signal, plus one inert dummy as
  a check on that expectation.
- **Stop** at 2,000 pairs if fewer than 3 live parameters exceed 2x their normalised null drift.
- Ships only stacked (expected +10-25).

### Step 5. Smaller items

- A judge from another engine family for the game-gap ladder (`../shogi-engines`), agreement
  checks only.
- Root stack parts not gated separately: not worth games.
- Keep every screen-vs-gate pair (screen estimate, gate result) in the research doc, to measure the
  filter's transfer over later sessions (Sol).
- For step 0b (Fable, optional): quiescence never consults the main table (`lean_inner` enters it
  before the probe, `lean.rs:300-323` vs `:351`; `QSlot` is per thread and holds no move); count
  horizon nodes with a usable main-table bound or move. Log `QSlot`'s overwrite rate at 1 s and
  5 s (2^18 slots per thread, always replaced, `lean.rs:1152`).
- Candidates for later, outside this plan (Gemini): a persistent helper thread pool owning its
  thread-local tables (replacing spawn-and-park, `lazy_smp.rs:97-106`); `lean_hist` learning from
  root cutoffs across iterations.

## 4. Not doing now

The 1.6 s vs 1.0 s match (r1): the two figures it was to reconcile do not conflict (F8), and
ungated speed work leaves it nothing to decide; `tu4.py` at byoyomi 1000 vs 1600 gives time→depth in
10 minutes if wanted. Network work (F15) until step 0a says the search gap at equal evaluation is
small. A YaneuraOu build with node-type counters (needs a source build and the user's approval).

## 5. Machine cost and odds

| Step | Mac | PC | Clocked games? | Outcome |
|---|---|---|---|---|
| 0b, 0c | ~50 min | — | no | orders step 2a |
| 0a | ~2.5-3 h (from pilot; ~290k nps on the testbed, F17), load-immune | — | no | k: sizes step 2 |
| 1 Screen calibration (r6) | ~4.3 h (2.1 h if improving stops it), load-immune | — | no | a validated 400k filter (~65-70%), or the 250 ms fallback |
| 2a Candidates + stack confirmation (r6) | ≤ 18 h incl. 7.3 h stack confirmation at 800k, load-immune, in scheduled chunks | — | no | a confirmed stack (≤ 2 carries + candidate 0), or none |
| YaneuraOu guard (r6, replaces 2c') | ~1.5-2 h after a gate pass, load-immune | — | no | no ship if stack − baseline < −29 |
| 2c Gate | ~3-5 h pooled | same | yes | ~30-40% given a stack |
| 3 Speed | minutes per change | differential tests | no | ships into baseline |
| 4 SPSA v2 | from pilot, ~7 h expected | or PC | no | stacked only |

Load-immune runs still slow the user's own games; they run in windows the user picks.

## 7. Results

### Steps 0b and 0c (2026-11-11)

Diagnostic build: detached worktree `yse-worktrees/s41-diag` at `6c611cf`, uncommitted counters
behind the existing `search_diag` switch (`DIAG_S41`, off in normal play), a `--moves-file` option
for `search-diag`, and a `SHOGI_S41_MATE_LOG` line for the root mate pre-search. Built into the
scratchpad. Positions: 40 middlegames (11-08 and 11-01 user losses, plies 40-76 every 4; gate-2
games 1-10, plies 50 and 70). Machine load 2.5-5 from other work during 0c.

**0b, 1 thread, depth 12, all 40 positions (12.9M nodes):**
- Quiescence is **44%** of nodes, not ~75% (F5 is out of date). By quiescence ply: 0: 53%, 1: 20%,
  2: 12%, 3+: 14%. Stand-pat cuts 40% of quiescence nodes; its table returns 11%.
- **Check extensions:** 245k applied, none refused by the path cap; **25.8%** of interior nodes
  have at least one check extension on their path (1: 12.8%, 2: 6.6%, 3+: 6.3%), and **8.2%** lie
  at or beyond the iteration's nominal depth.
- Work under each move category, share of the band's work (remaining depth 1-3 | 4-7 | 8+):
  move 1 44 | 48 | 55%; moves 2-3 17 | 14 | 11%; late quiet (LMR-eligible) 20 | 25 | 26%;
  **late non-losing checks (extended, never reduced) 7.9 | 4.9 | 1.8%**; late losing checks
  1.5 | 1.1 | 1.5%; late captures SEE<0 2.9 | 2.5 | 1.1%; late promotions 2.5 | 1.1 | 0.4%;
  evasions 4.3 | 2.3 | 2.0%.
- Interior nodes in check 5.7% (mean 6.0 legal moves).
- Horizon nodes with a main-table entry: 1.2% (quiescence consulting the main table would gain
  little). `QSlot` stores that overwrite another key: 91% (stale entries from earlier searches
  included, so not by itself thrashing).

**0c, the 11-08 game, plies 30-69 in one process, 1 s byoyomi, MoveOverhead 100, 256 MB:**

| Run | Completed depth (mean) | nps (mean) |
|---|---|---|
| 4 threads | 13.20 | 1.35M |
| 1 thread | 12.12 | 368k |
| 1 thread, losing-check pruning off | 11.88 | 399k (+8%) |
| 1 thread, improving flag 7 | 13.03 | 368k |
| 1 thread, root stack 0 | 11.28 | 378k |
| 1 thread, root stack 23 | 11.93 | 376k |

- 4 threads gain **+1.07 ply** over 1 (3.67x nps): at the ~1-ply line, so Lazy SMP is not
  obviously broken; a low-priority candidate. The last completed iteration ends at ~532 ms of the
  move's ~880 ms (4 threads).
- Root mate pre-search: mean ~50 µs, max 0.7 ms per move, 1-8 nodes. Negligible.
- nps noise between equivalent runs ~2%. **Losing-check pruning off moves nps +8%**, above the 3%
  rule, so that control runs at an equal-time node budget (§3 step 2a item 7). Improving and root
  stack do not move nps.
- Profile (macOS `sample`, 25 s, 4 threads, idle excluded): NNUE forward 22.7%, attack and
  legality tests 18.9% (`is_square_attacked_by`, attack patterns, the move-test filter), SEE 13.7%,
  malloc/memmove/memset 8.9%, ordering and sorting 7.9%, move generation 7.9%, NNUE incremental
  5.5%, hashing and repetition keys 4.7%, make/unmake 4.5%.

**What it changes:** step 2a order stays check extensions (1) and singular extension with them
(2) first, then LMR scope (3: late checks 5-8% of shallow work, losing captures and promotions
3-5%), evasion pruning (4: 2-4%), and quiescence bounds (5) drop to last among the fixed-node items
(depth 3+ is ~6% of all nodes). Step 3's speed targets: attack/legality tests, SEE (computed in
ordering and again in pruning) and allocation, before NNUE forward.

### Step 0a (2026-11-11): work-equivalence at equal evaluation

Scripts and games: `matches/s41/0a/` (`run_rung*.sh`, `pool.py`, one CSV per seed). Ours: the
diagnostic build of `6c611cf` with `--features yo-diagnostic` (default path checked identical to
`6c611cf`: depth and nodes on 8 positions at depth 10), `SHOGI_YO_NNUE` = halfkpvm (loaded,
checked through a stderr wrapper; start position 75 cp), root mate solver at 9k nodes. YaneuraOu
V9 + halfkpvm at 100k nodes. Both 1 thread, no book, 8 random opening plies, 12-seed blocks
(10 at 32x). Pilot (20 games at 4x): 20.0%, ~2.5 min per game.

| Row | Seeds | W-D-L | Score | Elo ± SE |
|---|---|---|---|---|
| halfkpvm 4x | 43200-11 | 7-2-87 | 8.3% | −417 ± 61 |
| halfkpvm 8x | 43300-11 | 16-3-77 | 18.2% | −261 ± 47 |
| halfkpvm 16x | 43400-11 | 34-2-60 | 36.5% | −97 ± 37 |
| halfkpvm 32x | 43500-09 | 29-2-29 | 50.0% | 0 ± 44 |
| a1 32x, solver 9k | 43600-09 | 21-1-38 | 35.8% | −101 ± 42 |
| a1 32x, solver 200k | 43600-09 | 21-1-38 | 35.8% | −101 ± 46 |
| **11-05 engine** (`usi-engine-d09cd8e-a1`), a1 32x, solver 200k | 43600-09 | 30-1-29 | 50.8% | +6 ± 40 |

- **k ≈ 29x** (weighted fit over the four halfkpvm rungs: +140 Elo per doubling; 95% 21x-40x).
  This meets the plan's k ≥ 16 trigger: the plan goes back to the council.
- **a1 vs halfkpvm in our search:** −101 vs 0 at 32x, 1.66 SE (unpaired), under the 2 SE line but
  against N1's "a1 ≈ halfkpvm".
- **Solver budget is not a lever:** 200k vs 9k gave the same score; 37 of 60 games identical.
- **Today's engine vs the 11-05 engine, on the same 30 opening pairs: −0.150 ± 0.09 in score
  (SE over pairs, council correction; the games diverge by plies 8-12, so the pairing adds
  little), about −105 Elo, ~1.65 SE.** The S40-41 ships gained ~+110-150 in self-play gates (1 s,
  4 threads); against YaneuraOu at fixed nodes, 1 thread, they show no gain and perhaps a loss.
- **Testbed validated:** production `engines/usi-engine-6c611cf` replays the first pair of each
  seed 43600-09 identically to the testbed's 200k-solver games (20/20 move for move).
- **Hand rule cost (offline, 40 positions, depth 12, 1 thread):** with superior/inferior
  repetition on (default) the tree is 12.90M nodes; with `SHOGI_NO_HAND_REPETITION=1` (main search
  and quiescence) 10.24M, so the rule costs **+26% nodes**. 0.54% of main-table stores (12,351)
  carry a hand-repetition-range score (|s| ≥ 29,000, not mate); the store at `lean.rs:1001-1013`
  has no repetition guard, so these path-dependent bounds can be reused on other paths.
- **Node accounting checked:** at `go nodes 3200000` on 10 positions both engines reach the same
  depth (17.5 vs 17.4) at similar nps (374k vs 355k), so N buys the same search. Today's root
  stack depth gain is offset on these positions by the nodes the later correctness fixes add
  (round 5-7's quiescence hand rule: +33% all nodes at depth 9).

### After the 0a council (2026-11-11): offline checks and run 1

Ordered by the regular council plus GPT-6 Astra and Claude Fable 5.2 (RESEARCH_S40, "Council
(2026-11-11, result reading ...)").

**Nodes to depth 12** (`s41_ntd.py`: 40 middlegames, fresh `usinewgame` each, 1 thread, mate
pre-search off), geometric mean and ratio to the previous binary:

| Binary | Nodes | Ratio |
|---|---|---|
| `d09cd8e-a1` (11-05) | 245.8k | |
| `8934982` | 247.5k | 1.01 (0.88-1.15) |
| `4dfe9c4` (root stack) | 161.4k | **0.65** (0.57-0.75) |
| `df00d98` (code review) | 185.8k | 1.15 (1.00-1.33) |
| `6c611cf` (rounds 5-7) | 176.5k | 0.95 (0.86-1.05) |

Today reaches depth 12 on **0.72x** the 11-05 engine's nodes.

**Hand rule** (diagnostic build, new `SHOGI_NO_Q_HAND_REPETITION` for quiescence only; 40 positions,
depth 12): all on 12.90M nodes; quiescence part off 12.12M (the quiescence part costs +6.5%);
all off 10.24M (+26% for the whole rule; the main-search part predates 11-05, `d09cd8e`
`lean.rs:216-217`). **1.31% of main-table cutoffs (3,443 of 263k) use a hand-repetition-range
score** (0.93% with the quiescence part off): path-dependent bounds reused by other paths. A
correctness candidate for step 2, long-standing, not a cause of the 0a result.

**Run 1** (`matches/s41/run1/`): production `6c611cf` vs `d09cd8e-a1`, self-play, 1 thread,
400k nodes each, a1, 12 seeds × 26 games: **193-8-111, 63.1%, +94 ± 18 Elo.** By the rule fixed
in advance (≥ +40), self-play sees a gain that YaneuraOu did not (0a: about −105 ± 63 at 32x), so
run 2 runs. The gain is visible at 1 thread, so "the ships' gains lived in the 4-thread path" does
not explain the disagreement; the opponent or the depth does. Run 2 plus a 16x self-play row
(the same node budget as run 2) separate those two.

### Run 2 and the 16x self-play row (2026-11-11)

Scripts `matches/s41/run2/` (`run2.sh`, `paired.py`); YaneuraOu games in `matches/s41/0a/r2*_16x_s*`.

| Same change, today (`6c611cf`) − 11-05 (`d09cd8e-a1`), a1, 1 thread, fixed nodes | Games | Elo ± SE |
|---|---|---|
| Self-play, 400k nodes each (run 1) | 312 | +94 ± 18 |
| Self-play, 1.6M nodes each | 168 | +56 ± 26 |
| vs YaneuraOu (100k), ours 1.6M, paired on 120 opening pairs (run 2): 38.3% vs 34.4% | 240 + 240 | +30 ± 29 (95% −28 .. +87) |
| vs YaneuraOu (100k), ours 3.2M, 30 pairs (0a) | 60 + 60 | −105 ± 63 |

- **Run 2 verdict (rule fixed in advance): gross regression ruled out.** Pooled with 0a (inverse
  variance), today vs 11-05 against YaneuraOu is about **+6 ± 26**: no loss and no clear gain.
  The 0a "regression" was mostly noise in one 60-game row.
- **The self-play gain appears to shrink with depth:** +94 at 400k, +56 at 1.6M (difference
  38 ± 32, 1.2 SE: suggestive, not established; council round 1, Sol). At the same budget,
  self-play (+56 ± 26) and YaneuraOu (+30 ± 29) differ by 26 ± 39, not significantly. So the
  disagreement looks more like **depth** than the opponent, though neither is established.
- **What it changes:** the k ≈ 29x gap stands, and F18 (parity near 6-10x) was wrong from the
  start. Screens at shallow budgets overstate: step 1's 100k-node screen (≈ depth 10-11) should
  rise to at least 400k with the stack checked at 1.6M, and each stack gets the run-2-style
  YaneuraOu check at 16x (240 + 240 games, ~3.5 h, paired, SE ~29) before its gate.

### Depth vs nodes (2026-11-12, council round 1 check)

`s41_seq.py`, diagnostic build (default path identical to `6c611cf`), the 11-08 game plies 30-69
in one process (table kept across moves, as in games), 1 thread, `go nodes N`, solver 9k; mean
completed depth: 400k 12.68, 600k 13.75, 800k 14.18, 1.2M 15.22, 1.6M 15.88 (~1.5 ply per
doubling at 400k-800k). The gate's 13.20 (0c, 1 s, 4 threads) is reached at ≈ 510k. nps ~350k.

### Step 1 (2026-11-12, started): screen calibration at 400k

Scripts `matches/s41/step1/` (`run_ctrl.sh`, `chain.sh`, `chain2.sh`); `usi_match2` (s41-diag:
per-game think time in the CSV, mean think time in the summary); `0a/pool.py` gained `--first N`
and `--halves` (checked on run 1: unchanged totals; halves +98 ± 29 | +89 ± 24, z +0.26).
- **Pilot** (A/A, diagnostic build vs itself, 400k, solver 9k, seeds 44500-44509 × 2 games):
  **not deterministic**: 1 of 10 colour-swapped pairs identical, 5 of 10 pairs scored ½. Mean
  think A 1032 ms, B 1032 ms. So the A/A row runs (acceptance (c)), placed last and only if
  improving < 0 and losing-check > 0 (`chain2.sh`), so a failing step 1 does not spend it.
  Step 1 cost if all three run: ~6.4 h. **Moved to the PC** (the user made both machines
  available all day): the Mac's `chain2.sh` was stopped and the A/A row runs on the PC from 05:59
  (`pc_aa.sh`, `hold_pc_aa.sh`; same source, built there as `yse-s41diag`, a1 md5 identical), so
  step 1's wall time is ~4.2 h if it is needed, and nothing is lost if improving stops step 1.
- **Mac vs PC (found while preparing the PC):** `go depth 10` on 8 positions gives different trees
  and sometimes different best moves (e.g. `u1108_p40`: Mac 137k nodes, 7f7e; PC 56k, 3f2f; at depth
  3 the PVs already differ). Not the evaluation or move generation: `eval_dump` (new diagnostic
  binary in s41-diag) gives identical from-scratch evaluations and legal-move counts on 2,360
  positions and all their children; not the LMR table (no entry within 1e-9 of a rounding
  boundary). Cause open (candidates: SEE or attack code with platform-specific paths, incremental
  evaluation updates). Each run stays on one machine, so no comparison mixes platforms; the
  Mac/PC differential tests of step 3 now have a concrete failing case. On the Mac, `go depth`
  repeats exactly across runs; a position searched first in a process differs by ~256 nodes from
  the same position searched after another (some state survives `usinewgame`).

- **Improving 7 vs 0** (400k, seeds 44400-44411, 600 games, ended 07:50): 293-31-276, 51.4%,
  **+10 ± 13** (95% −16 .. +36), halves −1 ± 19 | +21 ± 18 (z −0.84); mean think ratio 0.990.
  **Acceptance (b) fails** (it required < 0): by the rule fixed in advance step 1 stops, the 400k
  screen is unvalidated, losing-check was not run. The control's gate value on today's engine is
  itself unknown (F4's −34.9 ± 53.5 was a stopped SPRT on the S35 engine; pooled −28 ± 29), so
  this does not say which is wrong, the screen or the control.
- **A/A on the PC, stopped at 324 of 600 games** when step 1 stopped: 136-14-168, 45.0%,
  **−35 ± 17** (95% −69 .. −1), mean think ratio 1.008. An A/A has a true value of 0, so this is
  either chance (2.0 SE, p ≈ 0.04) or a bias between `usi_match`'s engine slots. Game endings show
  nothing odd (no-legal-moves wins, 10 repetition and 5 move-limit draws). Unresolved.

### After the step 1 council (2026-11-12): two bugs found, A/A settled

Council (Opus, Sol, Gemini; RESEARCH_S40 "Council (2026-11-12, result reading: step 1)").
- **Mac/PC difference = a missing file, not the platform** (Gemini): `magic_table.bin` is
  gitignored (`.gitignore:51`), so the `git ls-files` tarball left it out of the PC tree. Without
  it the engine silently ray-casts move generation and **SEE sees no sliding attackers**
  (`see_calculation.rs:305-307` returns early). Checked both ways: the Mac with
  `SHOGI_MAGIC_TABLE_PATH=/nonexistent` reproduces the PC's `go depth 10` numbers exactly; the PC
  with the file (copied from `yse-s42`, md5 identical) reproduces the Mac's exactly. The past PC
  gate trees (`yse-s40`, `yse-s42`) have the file. The PC A/A row above ran without it (both
  sides), so it is void. Engine fix candidate: fail loudly, or give SEE a ray-cast fallback.
- **Node-count leak** (Sol, Opus, Gemini; checked): nodes reach the global counter in batches of
  256 (`statistics.rs:162-167`), and nothing clears the unflushed remainder between searches, so
  0-255 nodes of the previous search count toward the next one's `go nodes` budget. This is the
  ~256-node difference and the pilot's non-determinism. Diagnostic fix behind
  `SHOGI_S41_NODE_FIX=1` (`discard_pending_global_nodes` after `begin_search`; default path
  unchanged: depth 10 on 8 positions identical). With it, a position searched after another
  matches the fresh search (220,928 nodes both ways).
- **Determinism pilot with the fix** (A/A, 400k, seeds 44520-44529 × 2, Mac): **10 of 10 pairs
  move-for-move identical, every pair ½, 0.0 ± 0.0**; mean think ratio 1.002. The harness treats
  slots A and B alike; acceptance (c) is met. The PC's −35 ± 17 is recorded as chance on a void
  build.
- **Improving**: all three read +10 ± 13 as the control being stale (measured on the S35 engine,
  stopped SPRTs biased low), not the screen being blind; neither is established.

### Engine fixes (2026-11-12, commit `76fb942` on `main`, fast-forwarded from branch `s41-fixes`)

The user chose "fix bugs first". Worktree `yse-worktrees/s41-fixes`, built into the scratchpad.
- **Node remainder**: `SearchStatistics::discard_pending_global_nodes`, called right after
  `begin_search` in `IterativeDeepening::search` (helpers build a new engine every move, so only
  the main engine carries a remainder). Test `tests/node_budget_history.rs` (own binary; the
  counter is process-global): a 30k-node search after a depth-5 search that left a 179-node
  remainder must stop where a fresh engine stops; fails without the fix (1,820 vs 1,865 main
  nodes in the cut iteration). A `go nodes` search leaves only 0-3 nodes (it stops right after a
  flush); that was enough to make A/A pairs diverge.
- **SEE without the magic table**: `add_sliders` ray-casts the rook and bishop rays
  (`bitboards::raycast_slider_attacks`) instead of returning; `isready` prints
  `info string WARNING: no magic table at ...` when the table is not loaded
  (`bitboards::magic_table_warning`). Test `see_without_magic_table_matches_table` (lib): SEE
  with forced ray-casting equals SEE with the table on every legal move along 30 random games
  (>10,000 moves with sliders among the attackers); fails without the fix (2h7h). Not fixed:
  without the table, move generation still emits sliding moves in another order (scalar path),
  so the search still differs; the warning makes that visible.
- **Checks**: `search_bench --depth 9` (34 positions, a1, 256 MB, no mate search) fingerprint
  byte-identical to `6c611cf`, all_nodes 1,643,135 both. `go depth 10` on 8 positions in one
  process: same best moves as `6c611cf`; positions after the first report 256 fewer nodes,
  equal to a fresh search. `cargo test --lib` 1163 passed; `board_semantics`,
  `move_legality_tests`, `node_budget_history` pass; `search_semantics` and `nnue_semantics` do
  not compile on `main` either (stale `NNUEWeights` initialisers, pre-existing). In timed play
  neither change alters the search with the table present (the remainder only fed `info
  nodes`), so no strength gate is needed.

### Step 1, second calibration (2026-11-12; user chose the council's option 1, Opus)

Fixed before any game. Controls with a known value on today's engine replace the stale improving
control (all three members read +10 ± 13 as a stale control). Both engines: the diagnostic build
(`usi-engine-diag2`: `6c611cf` + S41 diagnostics, default path identical) with
`SHOGI_S41_NODE_FIX=1` (the `76fb942` node fix, so games are deterministic) and
`SHOGI_MATE_NODES=9000`; a1, 1 thread, 256 MB, no book, `go nodes`, 600 games per control
(12 seeds × 50), seed blocks assigned in §2 and unused so far.
- **Root stack off (A, `SHOGI_ROOT_STACK=0`) vs default (B)**, 400k each, seeds 44200-44211, on
  the Mac. Its gate value is known: +73 ± 55 at `4dfe9c4` (1 s, 4 threads; at 1 thread bits 8 and
  32 do nothing); 0c: +0.65 ply at 1 thread.
- **Losing-check pruning on (A) vs off (B)**, A 400k, B 432k (equal time: off is 8% faster, 0c),
  seeds 44300-44311, on the PC (same source, magic table present, checked against the Mac at
  `go depth 10` before the run).
- **Acceptance**: (a) run 1 > 1.65 SE: met (5.2 SE); (b′) root-stack-off estimate < −1.65 SE;
  (c) seed agreement, Σz² over run 1 and the two new runs (block halves) < χ²(3) = 7.815;
  (d′) losing-check estimate > 0. A/A: settled by determinism (pilot 10/10 identical).
  All met → the 400k screen is accepted for step 2a (elimination filter; tests sign and scale,
  not sensitivity to +10-20). Any failure → the plan's 250 ms fallback, and the user is told.
  This is a second calibration chosen after a failure: reported as such, with the first.
- **Root stack off vs default** (Mac, ended ~12:00): 270-19-311, 46.6%, **−24 ± 13** (95% −50 ..
  +2), **−1.85 SE: (b′) met**, narrowly; halves −21 ± 18 | −27 ± 19 (z +0.22); mean think ratio
  1.004. Smaller than the +73 gate value at 4 threads, as expected for a 1-thread, depth-limited
  screen (and consistent with the depth fade). Run 1's halves: z +0.26.
- **Losing-check on (400k) vs off (432k)** (PC, ended 13:38): 340-28-232, 59.0%, **+63 ± 13**
  (95% +38 .. +88): (d′) met; halves +42 ± 17 | +85 ± 19 (z −1.67); mean think ratio 1.004 (the
  equal-time budget worked, and the `.bat` wrapper applied the switch). Games in
  `matches/s41/step1/pc_qplc/`.
- **Seed agreement**: Σz² = 0.26² + 0.22² + 1.67² = **2.91 < 7.815**: (c) met.
- **Verdict: all acceptance rules met; the 400k 1-thread fixed-node screen is accepted for step
  2a** (an elimination filter that ranked a known gain, a known loss and the ship stack right;
  it tests sign and scale, not sensitivity to +10-20). Reported with the first calibration, whose
  improving control failed (b). Step 1 cost: ~2.1 h Mac (improving) + 2.1 h Mac (root stack) +
  3.6 h PC (losing-check) + pilots; the Mac and PC builds are identical (`go depth 10`), so step
  2a can run on both, the PC at ~60% of the Mac's per-process speed.

### Step 2a (2026-11-12, started)

Branch `s41-cand` (worktree `yse-worktrees/s41-cand`, from `main` `76fb942`): `SHOGI_MATE_NODES`
plus environment switches for the candidates, all off by default (depth-9 fingerprint
byte-identical to `main` with every switch off). Runner `matches/s41/step2/screen.sh` (Mac) and
`pc_screen.sh` + `hold_pc_screen.sh` (PC, `yse-s41cand`, built from the same source with the
magic table; `go depth 10` identical to the Mac with a switch off and on).

**Offline selection** (40 middlegames; nodes to depth 12, geometric mean, ratio to base 176k;
nps on the 40-move sequence at 400k, base 375k; depth reached at 400k, base 12.43):

| Setting (switch) | Nodes to d12 | nps | Depth at 400k | Screen budget |
|---|---|---|---|---|
| No check extensions (`SHOGI_NO_CHECKEXT=1`) | 0.95 | +2.6% | 12.78 | 400k |
| Cap 1 per path (`SHOGI_S41_CHECKEXT=1`) | 1.05 | | | not run |
| Cap 2 per path (`=2`) | 1.03 | | | not run |
| No extension for quiet checking drops (`=3`) | 0.97 | +1.8% | 12.85 | 400k |
| LMR on late quiet checks (`SHOGI_S41_LMR_SCOPE=1`) | 0.99 | +1.1% | 12.65 | 400k |
| LMR on late losing captures and promotions (`=2`) | 0.96 | **+3.1%** | 13.15 | **412k** vs 400k (equal time) |
| Evasion LMP in check (`SHOGI_S41_EVASION=1`) | 0.95 | −0.1% | 12.82 | 400k |
| Singular extension (`SHOGI_LEAN_SE=1`) | **3.41** | −2.4% | 10.75 | 400k |

Family 1 plays "none" and "no drop extensions" (the caps grow the tree). **Candidate 0** (offline,
same 40 positions, game history included): of 2.08M main-table stores, 0.45% carry a hand-range
score; 1.5% come from subtrees whose hand repetition matched a position above the node (31,350
between the root and the node, 111 in game history), 70% of those with ordinary-valued scores;
24,171 matched only the node or below (path-independent). Setting (a) (no store in the hand
range): **+1.4% nodes**; setting (c′) (no store when matched above the node): **+12.7% nodes,
rejected by the 5% rule**. Check (ii) (re-searching cutoffs on their own path) was not run: it
needs each node's path rebuilt, too costly for a −10..+20 prior; the 312-game stage 1 decides.
So candidate 0 plays (a).

**Seed blocks**: candidate 0 46000-46011, no check extensions 46100-46111, no drop extensions
46200-46211, singular 46300-46311, LMR checks 46400-46411, LMR tactical 46500-46511, evasion
46600-46611; stack confirmation 45100-45111 (reserved).

**Runs**: no check extensions (Mac, from 13:58); no drop extensions (PC, from 14:00); candidate
0 queued on the Mac after it (`queue_mac.sh`, 312 games, build `usi-engine-cand2` with the
candidate 0 code, default path identical).

| Run | Machine | Stage 1 (312) | Final (600) | Disposition |
|---|---|---|---|---|
| No check extensions, 46100-11 | Mac | +3 ± 18 | 280-25-295, **−9 ± 13** (halves −9, −8); think ratio 0.985 | dropped (≤ +10) |
| No drop extensions, 46200-11 | PC | −3 ± 18; think 0.992 | 285-24-291, **−3 ± 13** (halves −5, −2); think 0.995 | dropped (≤ +10) |
| Candidate 0 (a), 46000-11 | Mac | 136-15-161, **−28 ± 19** (−1.46 SE; 95% −65 .. +9); halves −0 ± 28 / −56 ± 26; think 1.008 | (312 only) | **kept by the rule** (drop only below −1.5 SE); borderline, flagged to the user |
| Singular, today's extensions, 46300-11 | Mac | 175-14-123, **+58 ± 19** (95% +21 .. +96); think 1.012 | 342-28-230, **+66 ± 13** (95% +39 .. +92); halves +64 ± 19 / +67 ± 20; think 1.010 | **carried** (first carry) |
| LMR on late losing captures and promotions, 46500-11 (A 412k, equal time) | Mac | 172-8-132, **+45 ± 18** (95% +10 .. +79); think 1.000 | 312-24-264, **+28 ± 13** (95% +3 .. +53); halves **+60 ± 18 / −3 ± 18 (z +2.46)**; think 1.004 | **carried** (family 3's best; second carry: screening ends) |

Tie-break correction (20:41, before either LMR final): the two LMR runs are settings of one
family (item 3, a bit set of orthogonal sub-knobs). The plan's earlier rule applies: only the
best setting above +10 carries; combining them would need its own screen. That rule predates
the plan-order tie-break written at 19:32, which is withdrawn. If the better LMR setting
carries, it is the second carry and screening ends.

**Screening ended** at the second carry (22:15). The two LMR settings are effectively tied (+28
vs +25), and the carried one has seed halves that disagree (z +2.46). Evasion pruning was not
run. Step 2a Mac hours: 2.1 + 1.1 + 2.2 + 2.1 = 7.5; PC: ~3.6 + ~3.6.

**Stack confirmation** (fixed rules): stack = singular extension (`SHOGI_LEAN_SE=1`) + LMR on late
losing captures and promotions (`SHOGI_S41_LMR_SCOPE=2`) + candidate 0 (a) (`SHOGI_S41_HAND_TT=1`,
kept by its rule at −1.46 SE). nps at 800k on the 40-move sequence: base 393k, stack 372k
(−5.4%; depth 14.12 vs 12.03). Pilot, 24 games on seeds 45000-45011 × 2 at 800k: think ratio
**1.027 < 5%: no rescaling**. Run: 800k vs 800k, reserved seeds 45100-45111 × 84 = 1,008 games, Mac,
from 22:35 (`confirm.sh`), ~7.3 h. Estimate > +15 → 2b and the gate; else no gate, report.
- **Result** (ended 05:45): 582-56-370, 60.5%, **+74 ± 10** (95% +54 .. +95); halves +89 ± 15 |
  +59 ± 14 (z +1.45); think ratio 0.999. **Passes (> +15): on to 2b and the gate.** A
  1-thread fixed-node estimate: an upper bound on the gate (§3 2c), which sits between ~510k and
  1.35M nodes of work.
| LMR on late quiet checks, 46400-11 | PC | 162-17-133, **+32 ± 19** (95% −5 .. +70); think 0.999 | 306-31-263, **+25 ± 13** (95% −1 .. +51); halves +29 ± 20 / +21 ± 18; think 1.001 | > +10 but not the family's best: not carried |

### 2b and the gate (2026-11-13)

**Ship branch** `s41-ship` (worktree `yse-worktrees/s41-ship`, from `main` `76fb942`, uncommitted):
singular extension on by default (`SHOGI_LEAN_SE=0` off); LMR for late losing captures and late
non-capturing promotions (`lmr_tactical_candidate`; `SHOGI_LMR_TACTICAL=0` off); no main-table
store of hand-repetition-range scores (`SHOGI_HAND_TT_STORE=1` stores); singular extension's
global counters now only under the diagnostic switch. `usi_match`: per-game think time
(as `s41-diag`) and `--time-loss-ms`.
- **Equivalence**: `search_bench --depth 9` fingerprint of the ship build equals the screened
  stack (`s41-cand` with the three switches) exactly (all_nodes 2,851,655 both); with the three
  off-switches it equals `main` exactly.
- **Tests**: `main_search_does_not_store_hand_repetition_scores` (fails with
  `SHOGI_HAND_TT_STORE=1`, the old behaviour) and `lmr_tactical_candidates` (new predicate);
  `cargo test --lib` 1,165 passed.
- **4 threads, 1 s, 256 MB, MoveOverhead 100, the 11-08 game in one process** (plies 30-69 twice,
  70-109 once): no crash; longest move 858 ms (main 860); nps 1.34M vs 1.36M; completed depth
  10.9 vs 13.1 (singular extension searches narrower and deeper).
- **Time-loss rule** with a mock engine (relays `go depth 1`, holds the reply to a set time):
  900 ms held → measured mean 905, max 910 ms (harness latency ~5-10 ms); 1,300 ms held with
  `--time-loss-ms 100` → both games lost on time ("time (1303 ms)"). The gate uses 100 (a move
  over 1,100 ms loses).
- **Four answers** (as given to the user before the go-ahead): effect +30..+60 expected at the
  gate (screen-to-gate transfer mixed: root stack +24 screen vs +73 gate; ship stack +94 at 400k
  vs +110-150 gates), pass odds ~65-75%; state per thread (singular extension's excluded move is
  thread-local), checks above; conditions differ (1 thread fixed nodes vs 1 s, 4 threads;
  singular extension reads table entries the helpers share); addresses the equal-evaluation gap
  (k ≈ 29x); YaneuraOu guard after a pass.

**Gate 3** (user go-ahead 06:3x): ship vs `main` `76fb942`, pooled SPRT [0, 40] (`scripts/sprt.py`),
1 s byoyomi, 4 threads, 256 MB, MoveOverhead 100, time-loss 100 ms; Mac seeds 4131/4132, PC
4133/4134 (`yse-s41ship` vs `yse-s41main`, built from the same sources, magic table present,
`go depth 10` identical to the Mac), 250 games per stream, cap 1,000. Scripts `matches/s41/gate3/`;
started 06:39.
- **Result: PASS** at 07:29, LLR +2.97 (bounds ±2.94) after **122 games: 72-3-47, 60.2%, +72 ± 28**
  (95% +18 .. +126; a stopped SPRT, so biased upward). Mac 37-2-21 (+95 ± 39), PC 35-1-26
  (+51 ± 40). No time losses (121 games ended by mate, 2 repetition, 1 move limit); mean think
  ship 729 ms vs main 740 ms.
- **YaneuraOu guard** (after the pass, approved with the gate): `matches/s41/guard/guard.sh`, both
  rows on the Mac from 07:35 (~3 h), rule fixed in advance: ship − main < −29 → no ship, council.
  **Result** (10:37): ship 100-5-135, 42.7% (−51 ± 24 vs YaneuraOu); main 82-3-155, 34.8%
  (−109 ± 24); paired on 120 openings, **ship − main = +58 ± 30** (95% −2 .. +118): the guard
  passes (rule: < −29 stops). main scored 34.8% here vs 6c611cf's 38.3% in run 2 on the same
  seeds: noise (the node fix changes the games). Equal-evaluation k is not re-measured (a1 here).
- **Shipped** (user's go-ahead, 2026-11-13): commit `f3fd207` on `main` (fast-forward from
  `s41-ship`); the binary rebuilt from the commit searches identically to the gated one
  (`go depth 10`, 8 positions) and is installed as `engines/usi-engine-stable` (copies:
  `usi-engine-f3fd207`, `usi-engine-76fb942`, `usi-engine-6c611cf`).

## 6. Changes

**r6 round 2 (2026-11-12; Opus 8.9, Sol 8.9, Gemini 9.5):** acceptance (a) applies to run 1;
losing-check judged by sign only (Opus); stage continuation by `--games 50` and the first 26 CSV
rows per seed (Opus, Sol); 1,008 stack games (Sol, Opus); the stack holds at most two carries,
with a third triggering funded confirmations (Sol); candidate 0 outside the early stop and the
carry limit, one setting chosen offline, carries alone into the next stack, root store exempt,
(c)'s counter covers main search and quiescence and gates correction history, check (ii) also
samples ordinary-valued contaminated cutoffs, "dropped for budget" not "benign" (Opus, Sol,
Gemini); older rules settled (250 ms rescue and 2c' dropped; items 5-8 outside; Opus); losing-check
equal-time budgets written out (Gemini); 1.6M cost and total corrected (Sol). All five round-1
non-adoptions accepted by all three. **Round 3 (Sol 8.9, Opus 8.9):** third-carry confirmations
fully reserved (superseded below by Gemini: no three-carry branch); the 5% elapsed-time
check given outcomes inside the plan (mean think time in `usi_match`, node rescaling in the stack
confirmation, no gate if the stack is still off, nothing sent to the clock); (c)'s counter inside
`hand_repetition_score`, hand scores only; controls on the diagnostic build at solver 9k;
`pool.py --first 26`; odds' low end corrected. Gemini (9.6): no three-carry branch (it cannot be
funded: ≥ 19.9 h), the second carry ends screening; losing-check stop at ≤ 0; status line and §5
table updated. Round 4 (Sol 9.3, Opus 9.2): one rule for the stack confirmation's node budget
(a 24-game pilot measures the stack's time ratio, rescale if > 5%); labels, the fourth call site
and the third-carry wording fixed.

**r6 (2026-11-12, after runs 1-2; council round 1 on r6: Opus 8.4, Sol 8.3, Gemini 8.7; Muse
failed, billing):** screen at 400k (all); 1.6M depth check dropped (all); run 1 as the positive
control, root-stack control dropped, improving first with a stop rule, χ²(3) (Opus, Gemini, Sol);
two-stage runs at 312/600 games with a stop below −5 (Sol, Gemini); no per-carry confirmation
with ≤ 2 carries (Opus); budget counted in runs and every obligation funded before a run starts
(Sol, Opus, Gemini); stack confirmation at 800k, 1,000 games, no retry (Opus, Gemini, Sol); the
YaneuraOu guard after a gate pass, reusing run 2's baseline row, stop below −1 SE (Opus, Gemini);
candidate 0 respecified (no "none" flag; old setting b dropped as it leaks into singular extension
and ProbCut; setting c by subtree repetition; offline kill checks; regression tests) (all); the
fade softened to "suggestive" (Sol); measured depth vs nodes (Opus).
Not adopted: the guard's solver at 9k (Sol; reusing run 2's row needs 200k, and 0a found the two
identical); per-carry confirmations always (Sol, Gemini; the reserved-seed stack run is the
fresh-seed test when there are ≤ 2); candidate 0 into 2b without any games (Gemini; it changes the
search, so it gets a 312-game stage 1 and the stack confirmation); 600-800 stack games (Gemini;
1,000 at 800k costs what 600 at 1.6M did and halves the false-pass rate).

**Fable rating (2026-11-11, one independent review, 8.8):**
- Seed blocks of 12 per run, since `usi_match` plays one game at a time per process; blocks
  assigned for controls, 0a, 2a and the reserved stack confirmation.
- The root mate solver at a fixed 9k-node budget in fixed-node runs (a small diagnostic option),
  not off; check-extension items near zero get the clock screen before being dropped.
- Step 0a's trigger two-sided (k ≥ 16 also returns to the council); a closing fixed-node rung vs
  YaneuraOu with the confirmed stack (2c').
- Calibration failure odds 20-30% and validated-filter odds 70-80%; gate odds stated as
  conditional; mean think time added to `usi_match`; 0b items for quiescence and the main table.

**Astra rating (2026-11-11, one independent review, 8.9):**
- Quiescence-cache context for recapture-only search, with a regression test; a time-loss rule in
  `usi_match` checked with a mock engine before the gate; equal-time budgets logged and verified
  at both budgets, survivors off by > 5% clock-confirmed, the same policy for calibration
  controls; the a1 row can reopen evaluation work.

**Final (council round 5; not re-scored, each a member's own remaining point):**
- Post-round-5 (Opus): nps no longer routes a candidate to the clock; > 3% nps candidates run at
  equal-time node counts; F4 switches' nps measured in 0c; the stack's nps re-measured before its
  confirmation; k = 4 named; budget re-added (~13-21 h, the cap likely binds); headings fixed.
- Post-round-5 (Gemini): clock-confirmed items join at 2b/2c, outside the fixed-node stack
  confirmation; prefiltered positions read with `--stride 1`; candidates rescued at 400k carry only
  above +10.
- Post-round-5 (Sol): disjoint seed pairs per control for the joint test; the retry's removal pool
  excludes clock-screened members.

**r5 (council round 4):**
- Post-round-4 (Gemini): one setting per knob carries; the stack retry drops by latest estimate;
  A/A skipped if deterministic.
- Post-round-4 (Sol): fingerprint comparison by a whitelist of work fields (no `nps`/`all_nps`/`ms`),
  TIME-BOUND positions prefiltered from the corpus; the stack retry labelled exploratory.
- Post-round-4 (Opus): the deterministic branch uses one joint χ² seed-agreement test; failure odds
  ~20% in both branches; the depth check acts when gains grow with depth; items 7-8 have a length,
  thresholds and a 1 s confirmation; nps measured before screening decides fixed nodes vs clock;
  fresh-seed confirmation for items 4-5; the 18 h cap's stopping rule; 0a costed at ~2.5-3 h;
  the pilot's per-pair variance recorded.

**r4 (council round 3):**
- Step 2a: the 0 to +10 band is dropped for this budget; false carries computed (~24% per null
  candidate at SE 14), not promised; 400k confirmation on fresh seeds; stack confirmation on reserved
  seeds 4511/4512 with a +15 threshold and one drop-the-weakest retry (Sol, Opus, Gemini).
- Step 1: (a) one-sided 1.65 SE; root-stack control at 1,000 games; (d) reworded; A/A replaced by a
  cross-seed check if the pilot shows determinism; the depth check uses the difference and its SE;
  the chance that a working screen fails is stated (Opus, Gemini).
- SE defined: `usi_match` prints ± 1.96 SE (Opus, Gemini). Concurrency stated (Opus).
- Step 0a: rungs run until 50% is bracketed, both directions; bounds only against tested rungs;
  the k ≤ 2 trigger needs a bracketed k; 16x share corrected to ~57% (Sol, Opus).
- Fingerprint fields as `search_bench` actually prints them; TIME-BOUND positions excluded (Sol,
  Opus, Gemini). Mate-in-one's uncounted work described correctly (Sol).
- Optional ideas recorded in step 5 (Sol, Gemini).

**r3 (council round 2):**
- Mate pre-search off in every fixed-node run (F16; all three).
- Screen: fixed-length estimates, not SPRT verdicts; acceptance (a)-(d) fixed in advance, with
  improving required below 0 and A/A within 2 SE; unvalidated if it fails, no re-running until it
  passes; root stack control at 23 (1-thread bits) and at 100k and 400k (Opus, Gemini, Sol).
- The screen is an elimination filter; depth-dependent candidates and the stack confirm at 400k;
  mate-in-one and per-node-cost changes go on the clock; Lazy SMP candidates at 4 threads (Sol,
  Opus, Gemini).
- Carry threshold +10; total step 2a budget stated, with a cap (Opus, Gemini, Sol).
- Speed ships into the baseline before 2c; mate-solver fingerprint, legality and oracle tests,
  Mac and PC differential tests; `--depth 9`; `ms=` excluded; claim narrowed to tested coverage
  (Opus, Gemini, Sol).
- Step 0a: 4x first, bracketing rungs, k reported with uncertainty and out-of-range forms, used to
  size step 2 rather than gate it; a1 row at the rung nearest 50% (Opus, Sol, Gemini). 0b/0c first
  (Gemini).
- SPSA: drift normalised per parameter against expected null drift; correction-weight units checked
  (Sol, Gemini).
- 2c odds lowered to 30-40%.

Not adopted in round 2: Gemini's k ≥ 3 threshold (k is no longer a gate, so no threshold is
needed); Gemini's cap of the solver at node_limit/100 (switching it off is simpler and matches the
fingerprint).

**r2 (council round 1):**
- Step 0a: cross-engine nodes-to-depth with a 2.5x/1.5x rule replaced by a work-equivalence match
  at equal evaluation (Opus, Gemini, Sol: depth is not one unit; dead zone).
- Old step 1 (1.6 s vs 1.0 s) dropped (all three: F7/F8 do not conflict; decides nothing).
- Screen: move loss and nodes-to-depth were blind to two of three r1 controls and move loss was
  already shown useless in S35; replaced by fixed-node games calibrated on four controls,
  including a known FAIL and an A/A (Opus, Gemini, Sol).
- Kill rule: screen result, not nodes-to-depth (Sol, Opus: `SHOGI_IMPROVING`).
- New candidates: check extensions, LMR on checks and losing captures, evasion pruning, quiescence
  bounds (Opus, Gemini); ≤ 3 settings per feature (Sol).
- Step 3 ungated (Opus, Gemini, Sol); SEE-twice and double-sort items.
- Thread scaling and mate pre-search timing measured in 0c (Opus, Gemini).
- SPSA: cost corrected, pilot first, continuous parameters, `--corr`, dummy reference (Gemini, Sol).
- Explicit settings for every launch (Sol).
