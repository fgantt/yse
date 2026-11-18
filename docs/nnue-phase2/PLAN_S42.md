# Plan S42: re-screen the switched-off search features on the shipped engine

**Status**: r4 (final), 2026-11-15; council rounds 1 (Opus 7.0, Sol 8.0, Gemini 8.2), 2 (Gemini 9.5, Sol
8.9, Opus 8.8) and 3 (Gemini 9.8, Opus 9.3, Sol 9.2); §9. **Round 1 ran 2026-11-15/16: stack confirmed
+63 ± 11 at 800k, gate FAIL at 802 games (+11 ± 12); not shipped** (§10). Round 2 needs a new go-ahead.
**Baseline**: `f3fd207` (network a1), installed as `engines/usi-engine-stable` since 2026-11-13.
**Sources**: `PLAN_S41.md` (§3 r6 for the screen's rules, §7 for every result cited as S41),
`SESSION_LOG_035.md` ("Screens on top of the history ordering"), `RESEARCH_S40.md` §2c.

## 1. Why this plan

| # | Fact | Source |
|---|---|---|
| G1 | The search is still the gap. With both engines on YaneuraOu's network, our search needs about 29x YaneuraOu's nodes for an even score (95% 21-40x), measured before `f3fd207`. Our network in our search is about level with theirs in ours | S41 §7 0a; F15 |
| G2 | `f3fd207` still scores **42.7%** (−51 ± 24) against YaneuraOu + halfkpvm at 100k when it gets 16x the nodes (1.6M) | S41 §7 guard |
| G3 | Singular extension had been switched off after a single S35 screen on an older engine. Re-screened on today's engine, it gained **+66 ± 13** (400k), and the stack built on it gained +74 ± 10 (800k) and +72 ± 28 at the gate | S41 §7 2a, gate 3 |
| G4 | The S35 screens of IIR, ProbCut, mate-in-one and history LMR ran on a testbed with three of four helpers' history reset every move, no root stack, no singular extension and no node fix, at one setting each. None gained, so all stayed off. History pruning was never screened alone. These nulls are real data on a weaker engine, not nothing | S41 F11 |
| G5 | Singular extension changes the tree everything else works on. Completed depth on 1 thread at 400k: 12.68 before it, 10.75 with singular extension alone; at 800k the S41 stack completed 12.03 vs the baseline's 14.12. At the gate (1 s, 4 threads) the ship completed 10.9 vs 13.1. Results measured before it (LMR on quiet checks +25, evasion pruning never run) are out of date | S41 §7 2a, 2b |
| G6 | Today's singular extension is the most aggressive of the S35 settings: depth ≥ 6 and a margin of **2 cp per ply** (`lean.rs:1046-1053`, `:645-691`). The same margin sets both the extension (`v < sing_beta`) and the multicut (`sing_beta >= beta` → return, `:685-689`). The extension is at most one ply and is shared with the check extension (`lean.rs:784-788`), so **a singular test on a table move that gives a non-losing check adds nothing** unless it ends in a multicut. There is no negative extension and no double extension | code (Gemini) |
| G7 | The 400k screen is validated: its controls came out right (root stack off −24 ± 13; losing-check +63 ± 13), fixed-node games are deterministic (10/10 identical A/A pairs), and it predicted the stack (+74 confirmation, +72 gate) | S41 §7 step 1, 2a |
| G8 | Screen depth roughly matches the gate: 1 thread at 400k completes 10.75 plies with singular extension alone (the shipped stack's figure is measured in step 0), and the gate at 1 s on 4 threads completes 10.9. Threads do not match (1 vs 4) | G5 |
| G9 | The screen's root mate solver runs at a fixed 9k nodes, as at the gate, through `SHOGI_MATE_NODES`. That option was only on the S41 diagnostic branches, which survive only as patches (`matches/s41/leftovers/`). It is **not on `main`** | code |
| G10 | Helper threads keep their history tables for the whole game: `lazy_smp.rs:150,191` call `lean_hist::unpark`/`park` (`lean_hist.rs:92-114`; the module comment at `:22-23` saying helpers start fresh is stale). The 1-thread vs 4-thread difference for history features is per-thread learning and the shared table, not cold tables | code (Opus, Sol, Gemini) |
| G11 | **A latent ProbCut bug** (off by default): a cut returns `v - (pc_beta - beta)` (`lean.rs:562`), so a hand-repetition score of 30,000 comes back as 29,794 and a mate's distance moves by 206 plies. `corr_learnable` (`lean.rs:1127-1130`) excludes only \|s\| ≥ 30,000, so a 29,794 score can reach correction history; `is_hand_repetition_score` (`:1091`) uses 29,000 | code (Gemini) |

The bet: S41 showed that a feature switched off on an older engine can be worth a lot on today's
engine. The remaining switched-off features cost little to screen, and the screen is now
trustworthy. If four families give nothing, that ends this budget, and the next plan should
change the search itself (§6).

## 2. Measurement settings (unchanged from S41 unless stated)

- **Screen**: 1 thread, `go nodes 400000`, 256 MB, no book, root mate solver at 9k, 12 processes
  with one seed each, colour-swapped pairs, SE over opening pairs (`matches/s41/0a/pool.py`,
  `--first` and `--halves`). Two stages: **312 games** (26 per seed); **stop if the estimate is
  below −5**; otherwise go on to **600** (50 per seed) and judge on all 600: **above +10
  carries**. Only the best setting of a family carries. At SE ≈ 13 a null candidate clears +10
  about 22% of the time, so a carry is weak evidence on its own; the confirmation on reserved
  seeds is what counts (Sol, Opus).
- **Equal time** (corrected, Sol): if a candidate's nps on the 40-move sequence differs from the
  baseline's by more than 3%, the **faster** side gets proportionally more nodes:
  N_candidate = 400k × nps_candidate / nps_baseline (as S41 gave the faster tactical-LMR build
  412k). Mean think-time ratio is recorded for every run; a ratio off by more than 5% sends that
  run to the council instead of into the stack.
- **Stack confirmation**: 800k nodes, **1,008 games** (84 per seed) on reserved block
  47900-47911, **split by seed between machines** (Opus): seeds 47900-47906 on the Mac (588
  games, ~4.3 h) and 47907-47911 on the PC (420 games, ~5 h). Fixed-node games are
  machine-independent once the PC's `go depth 10` counts match the Mac's. A pilot of 24 games on
  47800-47811 first; think ratio off by more than 5% → node budget rescaled. **Above +15 → gate.**
  The confirmation's own ratio must be within 5%, or no gate. No retry.
- **Gate**: pooled SPRT [0, 40], 1 s byoyomi, 4 threads, 256 MB, MoveOverhead 100,
  `--time-loss-ms 100`, Mac + PC (`matches/s41/gate3/` scripts), Mac seeds 4231/4232, PC
  4233/4234. **Cap 1,000 games; no PASS by then → no ship** (Sol, Opus).
- **YaneuraOu guard** after a gate pass: the stack vs YaneuraOu + halfkpvm at 100k, ours at 1.6M,
  solver 200k, seeds 43800-43811, 240 games, paired by opening with S41's `g3ship_16x` row
  (`f3fd207` itself, `matches/s41/guard/guard.sh:10`). The replay check moves into step 0 (item
  7). **Rule: stack − baseline < −29 (−1 SE) → no ship, council.**
- **PC**: magic table copied and `go depth 10` node counts compared with the Mac on 8 positions
  before any PC game counts.
- **Seed blocks**: screens 48000-48011, 48100-48111, ... one block per run in the order of §5
  step 1 (up to 48900); Check 0 49000-49011; pilot 47800-47811; stack confirmation
  **47900-47911 (reserved)**; gate 4231-4234.

## 3. Candidates

Each family is one question. The 40-position diagnostic (step 0) picks settings and applies the
kill rules below, all fixed now. Nodes-to-depth is a diagnostic only: singular extension needed
3.4x the nodes to reach a depth and still gained +66 (G3). Priors are judgment, spread wide.

| # | Family | Settings | Code | Prior (screen Elo) | Kill before games |
|---|---|---|---|---|---|
| E | **LMR on late quiet checks, on top of the shipped tactical LMR** | port S41's `LMR_SCOPE` bit 1 only; keeps today's `lmr_tactical_candidate`. Checking captures and promotions stay unreduced; losing quiet checks become reducible | port from patch | −5 .. +25 | none (its S41 run fired) |
| A | **Singular extension settings** | A1: one margin from the grid {4, 8, 16} (margin 0 measured as a reference), the one whose extension rate is nearest half of margin 2's, at depth ≥ 6. A2: **a singular table move that also gives a non-losing check extends two plies**, so the test at G6 no longer adds nothing. Within the existing per-path budget: `want` = check extension (0/1) + singular (0/1), `room` = `current_depth.max(1) − check_extensions_on_path` (saturating), `ext = min(want, room)`; with one ply of room left it still extends one ply, as today (Gemini, Opus, Sol). The counter adds and subtracts the same `ext` (`lean.rs:803,934`) | A1 env only; A2 new, ~10 lines + tests | −30 .. +30 | A1: none. A2: if **actual two-ply extensions** (counted where `ext == 2` is applied) are under 5% of singular extensions whose table move was searched (Sol, Opus). Also counted, with A2 on and off: nodes where `room` is 0, since A2 spends the per-path budget faster and later check extensions on those lines lose room (Opus) |
| B | **Singular extension: shallower table move** | when the singular test fails (v ≥ sing_beta) without a multicut, at a non-PV node, and either the table score ≥ beta or it is a cut node, the table move is searched **one ply shallower**. Not for a table move that gives check, not at PV nodes (singular extension never runs at excluded ones, `:645`). The −1 changes `new_depth` only and never touches the unsigned extension counter (`lean.rs:803,934`; Opus, Gemini). The "table score ≥ beta" band is [beta, beta + margin·depth), so it grows with the margin | new, ~25 lines + tests | −15 .. +20 | if it fires on under 2% of singular tests, **measured at the margin B will run with** (margin 2, or A1's if A carries; Opus) |
| C | **IIR** | `SHOGI_LEAN_IIR=1` (`lean.rs:571-575`): a node at depth ≥ 4 with no table move, PV and in-check nodes included, is searched a ply shallower. A node has no table move only on a full table miss, since fail-low stores carry the first move (`:1018`). IIR and singular extension never act at the same node (one needs a table move, the other none); they interact through shallower stores (`:648` needs e.depth ≥ depth − 3) | exists | −10 .. +20 | if it fires on no nodes; singular tests per node recorded with it on |
| F | **Evasion late-move pruning** | S41's `SHOGI_S41_EVASION=1` (depth ≤ 3, past 2 + depth moves, not PV, once a non-mated score is found), plus `excluded.is_none()`, and quiet evasions that give check are not pruned (Sol) | port from patch + change | −15 .. +15 | if it prunes nothing |
| D | **ProbCut** | `SHOGI_LEAN_PROBCUT=1` (depth ≥ 5, margin 206; `lean.rs:494-562`), **with G11 fixed, all under ProbCut's own switch**: a cut with \|v\| ≥ 29,000 (hand-repetition or mate range, either sign) returns beta; ProbCut is not tried when \|beta\| ≥ 29,000 (entry test `:494`; Opus, Gemini); and `corr_learnable` uses the 29,000 range while the switch is on (Sol: the range change alters learning for [29,000, 30,000), so it stays off the default path). Its table read suppresses an attempt only on deep enough Exact or UpperBound entries below pc_beta; `prunable` (`:396`) keeps it out of PV, in-check and excluded nodes | exists + fix | −15 .. +20 | if it cuts on no nodes |
| G | **History in reductions and pruning** | G1 `SHOGI_LEAN_HIST=3` (history-adjusted LMR, `lean.rs:866-868`); G2 `=5` (history pruning, `:734-740`). Parameters `lmr_hist_div` 8123 and `hist_prune_mul` 3584 were set while the code was off | exists | −15 .. +15 | none |

**Deferred** (not in round 1): mate in one (H, `SHOGI_LEAN_MATE1`): a make-and-test of every
checking move at every interior node, and S35 found it recovers 1 of 469 mate-in-3 misses (Gemini,
Sol); IID (exclusive with IIR); the S41 check-extension caps (they grew the tree, S41 §7 2a).

**G11 and the default path**: with ProbCut off, nothing produces a score in [29,000, 30,000):
reverse futility returns about the static evaluation (`lean.rs:421-425`) and hand repetitions
return exactly ±30,000 (`search_engine.rs:7284-7285`) (Opus). So the `corr_learnable` range
change would be inert there, but a matching fingerprint cannot show it; it therefore ships only
with D. Once D's cut returns beta, ProbCut itself can no longer produce such a score either, so
the range change only matters if a static evaluation reaches the band (it is clamped only to
±89,999, `lean.rs:388-393`; Opus, round 3). A diagnostic counter of stored and learned scores in
that band runs **with D off and on, on the 40 positions and over the 10-pair game pilot**: if it
reads 0 everywhere, the `corr_learnable` change is dropped from D; if not, that is a separate
finding about the default path (it would also mean `is_hand_repetition_score` misreads real
evaluations) and goes to the council. The default path of `s42-cand` with every switch off stays byte-identical to `f3fd207`, so
the screens compare against `f3fd207`'s behaviour (step 0 items 2 and 4).

**Check 0: the hand-rule table fix, optional.** It went in at −28 ± 19 (−1.46 SE, 312 games)
because only below −1.5 SE was it to be dropped (S41 §7 2a). Run only when the PC has nothing
else funded: `SHOGI_HAND_TT_STORE=1` (old behaviour) vs `f3fd207`, 600 games, block 49000-49011,
counted apart from the carry limit and the early stop. If the old behaviour scores above +15, the
council decides between the fix and a narrower one; restoring the old stores is not an option by
itself, since they are path-dependent. Otherwise the fix stays and its strength effect is
recorded as **unresolved** (Sol). Opus would drop it (only 0.45% of stores are affected); it is
kept as idle-PC filler.

## 4. Field priors and how well they transfer

- **Singular extension** (A, B): strong alpha-beta engines in chess and shogi use singular
  extension with graded extensions and with reductions of the table move when other moves also
  hold. Our own +66 at 400k is the best prior for A; B and A2 transfer less, since our version
  has one-ply extensions and a shared check-extension budget.
- **IIR and ProbCut** (C, D): common in the field. RESEARCH_S40 §2c's YaneuraOu figures (+40.6,
  +24.9) **compare whole development versions** at 2 threads and 1 s with a different network,
  not single features (Sol, from YaneuraOu's development log); they are not a prior for C or D.
  Our own S35 screens on the old testbed gave nothing.
- **History in LMR and pruning** (G): central in chess engines, but enabling it here uses
  parameter values never tested live; the field prior is for well-tuned versions.
- Gains have been smaller at deeper searches (S41: ships +94 at 400k, +56 at 1.6M; 1.2 SE), so
  screen estimates are likely to overstate the gate, though not a strict upper bound (Sol).

## 5. Steps

### Step 0: offline (no games; about a day of my work, about 2 h of machine time)

1. **Candidate branch** `s42-cand` from `f3fd207`, in a new worktree. Port from the S41 patches:
   `SHOGI_MATE_NODES` (G9), the diagnostic counters, E and F. Write A2, B and D's fix. Add
   counters: singular tests, extensions, multicuts, checking table moves among extensions, B's
   firings, ProbCut tries and cuts, IIR reductions, evasion prunes. Every switch is read once and
   off by default.
2. **Identity check**: `search_bench --depth 9` on the prefiltered 150 positions, work fields only
   (S41 §2), and the mate-solver fingerprint: all switches off must equal `f3fd207` exactly.
   **Any difference is found and fixed before games.**
3. **Tests**, each failing with its change removed: A2 (a checking singular move extends two
   plies; with one ply of room it extends one; with none, zero; the counter is restored after the
   move and after an aborted search); B (reduced only in its window; never at PV or for a
   checking move; the extension counter is unchanged after the move); D's fix (a cut with
   \|v\| ≥ 29,000 of either sign returns beta; no attempt when \|beta\| ≥ 29,000; `corr_learnable`
   at ±28,999, ±29,000 and mate scores, with the switch on and off; Sol); F (no prune at PV,
   excluded, for a checking evasion, or before a non-mated score); E's predicate. Board, path and
   counter restoration after an aborted search for B and D.
4. **Determinism pilots** (~20 min): `f3fd207` vs itself, and `s42-cand` with every switch off
   vs `f3fd207`, 10 pairs each at 400k with the solver at 9k (Opus: the bench does not cover hand
   repetitions, the table aged across moves or the mate solver as games use them). Every pair must
   be identical, or find out why before screening.
5. **40-position diagnostic** (S41's 40 middlegames, 1 thread; Sol: the existing switches and the
   A grid first, ~30-60 min, before B, F and A2 are written): for each setting, nps at 400k,
   completed depth at 400k, nodes to depth 10, and its counters. This picks A1's margin, applies
   the kill rules of §3, and sets equal-time budgets. It also gives the shipped stack's completed
   depth at 400k for G8. If A carries, B's firing rate is measured again at A1's margin before B
   starts (a few minutes; Opus).
6. **4-thread smoke test** for every setting going to games: 1 s, 4 threads, 256 MB, the 11-08
   game in one process. No crash and longest move under 1,000 ms are required; nps is recorded,
   not a reason to drop (Sol). Singular tests and extensions are logged per thread for A, B and C,
   since helpers' table entries change how often the test runs (Opus).
7. **Guard reuse**: with the candidate binary, `SHOGI_MATE_NODES=200000` gives the same 1.6M
   `go nodes` result as unset on 8 positions (Opus); the installed `f3fd207` replays the first
   10 pairs of `g3ship_16x` identically. Either fails → the guard's baseline row is re-run
   (+~1.7 h Mac, funded in §8).
8. **PC**: build `s42-cand` there with the magic table; `go depth 10` counts equal the Mac's.

**Step 0 stops** the plan if the identity check or the determinism pilot fails and isn't fixed
within a day (Opus).

### Step 1: round 1 screens (Mac and PC in parallel)

- **Order** (Opus, Gemini, Sol: the candidate with direct evidence first, distinct families
  early): **Mac: E, A1, A2, B, G1, G2. PC: C, F, D, then Check 0.** B is screened **against the
  baseline it would join**: if A carries, B runs as A + B vs A, not vs `f3fd207`, so it cannot
  inherit A's gain (Sol); if A has not carried, B vs `f3fd207`. A family's second setting runs
  even if its first was stopped, when the diagnostic shows they differ (A1 and A2 move in
  opposite directions; Sol).
- **Carry limit**: at most two carries per round; the second carry ends the round's screening and
  the stack confirmation runs (S41 rule 3). A run already in flight then: on the **PC**, finished
  if past stage 1 and recorded; on the **Mac**, stopped (the Mac's hours are the user's; Opus).
  Neither joins the stack; a finished run above +10 is screened again on the new baseline in
  round 2. Unfinished runs are recorded as "stopped for budget".
- **Early stop**: if the first **four families** run (E, A, C, F in this order, counting a family
  once all its funded settings have a result; Check 0 not counted; Opus, Gemini, Sol) give no
  estimate above +10, screening stops and the result is reported. It ends this budget; it does
  not show that every remaining feature is worthless (Sol). **Enforced** (Sol): a machine whose
  queue reaches a run beyond those four families (Mac B, G1, G2; PC D, Check 0) before all four
  have results may start it, but if the early stop then triggers, that run is stopped at once and
  recorded as "stopped for budget". No result from such a run counts. A run of this kind that
  **finishes** before the four families have results is **provisional** (Sol, round 3): it does
  not count toward the carry limit and cannot start the confirmation until E, A, C and F have
  results. The early stop is then decided first; if it fires, provisional results are discarded.
- **Budget** (one wording; Opus): screens have their own caps, **Mac ≤ 12 h, PC ≤ 14.4 h**; the
  confirmation, gate and guard are funded separately (§8). A run starts only if its machine's
  remaining screen budget covers its full 600 games. The Mac's six settings at 2.1 h make 12.6 h,
  so **G2 runs only if an earlier Mac run stopped at stage 1 or a setting was killed offline**.
- Measured costs (S41 §3 r6): 400k ≈ 1.1 h for 312 games and 2.1 h for 600 on the Mac; the PC
  took ~3.6 h per 600.

### Step 2: confirmation, gate, guard, ship

Stack confirmation (reserved block, split Mac/PC) → gate (above +15) → YaneuraOu guard (if PASS)
→ the user's go-ahead → commit to `main` with the switches made defaults (off-switches kept),
tests, and the stack's fingerprint checked against the screened build →
`engines/usi-engine-stable`.

Not adopted (Opus, round 1): gating only above +25. The confirmation runs on reserved seeds, so
the screen's selection bias does not reach it; a stack at +15..+25 is the gate's job to judge.

**A confirmed stack that fails the gate** (Opus, round 2): it is not shipped. It is recorded as a
candidate base: round 2 (a new go-ahead) may screen new families on top of it, and the combined
stack then gets one confirmation and one gate against the shipped engine. Nothing unconfirmed
ships, and a real +20 stack is not thrown away by the gate's ~61% miss rate at that size.

### Step 3: round 2 (a new go-ahead)

If round 1 ships, the families not yet run and any finished run above +10 are screened again on
the new baseline, with the same rules and a fresh budget. If a confirmed stack failed the gate,
round 2 builds on it as above. If round 1 confirms nothing, round 2 does not run; §6 applies.

## 6. What a null result means, and what comes after

If four families give nothing, toggling the remaining features is probably not where the gain
is, and the next lever needs new search structure. The two strongest options known now:
- **SPSA at fixed nodes**: tune the live search margins at 400k on 1 thread. It is free of machine
  load, unlike SPSA v1 (about 30 ms per move with five dead parameters; S41 F12).
- **Graded singular extension**: extensions by how far the other moves fall short, with new
  per-path budget rules (Sol: not a round 1 change).

Neither is in this plan.

## 7. The four questions (plan as a whole)

1. **Effect vs what the gate resolves**: per family −15 .. +30 at the screen, mostly near zero;
   with about seven settings at a +10 bar, at least one false carry is likely (Opus ~85%). A gate
   is reached only with a fresh-seed confirmation above +15 at 800k; the gate resolves +40 at
   ~90% and +20 at ~39% (S41 F14). A confirmed stack is more likely +15 .. +30 at the gate than
   S41's +72.
2. **Correctness and state lifetime in 4-thread games**: A1 only changes constants. A2 and B
   change depth through `new_depth` and the per-path budget, which are per thread; the excluded
   move is thread-local (`lean.rs:1095`). C changes depth on table misses, which are fewer with
   helpers filling the table: the gate's condition, not the screen's. D reads table bounds
   shared by helpers, and G11 is fixed before it plays. E and F are per node. G reads per-thread
   history, kept across moves by park/unpark (G10).
3. **Do the test conditions match the gate**: depth roughly matches (G8); threads do not (1 vs
   4). The confirmation at 800k sits one to two plies beyond the gate.
4. **Does it address the measured gap**: yes, through the search, where the 29x gap lives (G1).
   The guard measures transfer against YaneuraOu with a1; it does not re-measure the
   equal-evaluation gap (Sol).

**Pass odds** (round 1 council: Opus ~15% (10-20), Sol 8-18%, Gemini 25-30%; mine after their
points ~15-25%): a carry ~70%, of which a real one ~45-55%; the confirmation passes given a carry
~55-65%; the gate passes given a gate ~35-50%; plus a small chance of a large surprise like
singular extension in S41.

## 8. Machine cost

| Part | Mac | PC | Blocks the user's games? |
|---|---|---|---|
| Step 0 | ~2 h (diagnostics, pilot, replay) | 8-position check, build | lightly |
| Round 1 screens | ≤ 12 h | ≤ 14.4 h | yes; in windows the user picks (fixed-node runs are load-immune) |
| Stack confirmation | ~4.3 h | ~5 h | yes |
| Gate | ≤ ~7 h, pooled (cap 1,000 games; S41 ran 122 games in 50 min, ≈ 6.8 h per 1,000; Sol) | same | yes (clock games: no user games meanwhile) |
| Guard | ~1.7 h (one row), ~3.4 h if the baseline row is re-run | — | yes |

At most about **29 h of the Mac** (2 + 12 + 4.3 + 7 + 3.4) and **26 h of the PC** (14.4 + 5 + 7)
for round 1 (Sol: count every funded obligation). Expected much less: a run stopped at stage 1
costs about half, the early stop or the second carry ends screening, and most gates end well
before the cap.

## 9. Change log

- r1 (2026-11-15): first draft, after the S41 ship.
- r2 (2026-11-15), council round 1 (Opus 7.0, Sol 8.0, Gemini 8.2; briefs in
  `matches/s42/council_r1_brief.md`). Adopted: equal-time rule corrected (faster side gets more
  nodes; Sol); seed blocks moved so screens cannot reach the reserved blocks (Sol); B's −1 kept
  out of the unsigned extension counter, B's window widened to cut nodes, a firing-rate kill, and
  B screened against the baseline it would join (Opus, Gemini, Sol); A1 chosen by extension rate,
  A2 replaced by the check-plus-singular two-ply extension, which fixes G6's wasted test (Opus,
  Gemini); E first, H deferred, distinct families early, early stop counted by families (all);
  ProbCut's score shift and `corr_learnable`'s range (G11; Gemini, checked in code); F spares
  checking evasions and excluded nodes (Sol); G10, helpers keep history (all, checked in code);
  kill rules fixed before step 0 (Opus); confirmation split Mac/PC (Opus); in-flight runs
  finished on the PC only (Opus); gate cap verdict (Sol, Opus); guard checks moved into step 0
  (Opus); smoke test no longer drops on nps (Sol); field priors corrected (Sol); budget recounted
  (Sol); Check 0 optional, "unresolved" not "noise" (Sol). Not adopted: gate only above +25
  (Opus; step 2); G moved out of round 1 (Sol; G10 removes the main reason, and it runs last on
  the Mac); dropping Check 0 (Opus; kept as idle-PC filler). Checked and not held: Opus's reading
  that S41's 14.12 vs 12.03 contradicts the 2-ply cost (14.12 is the baseline, 12.03 the stack;
  S41 §7 "Stack confirmation"). Not checked: Sol's reading of YaneuraOu's development log (§4 now
  uses no figure from it).
- r3 (2026-11-15), council round 2 (Gemini 9.5, Sol 8.9, Opus 8.8; brief
  `matches/s42/council_r2_brief.md`). Adopted: A2 as `ext = min(want, room)`, so one ply of room
  still gives one ply, with its kill counted on actual two-ply extensions (Gemini, Opus, Sol);
  B's kill measured at the margin it runs with (Opus); D's guard on \|v\| ≥ 29,000 of either sign
  and on \|beta\| ≥ 29,000, and the `corr_learnable` range change moved under D's switch (Sol)
  with a band counter that must read 0 with D off (Opus); a second determinism pilot,
  `s42-cand` all-off vs `f3fd207` (Opus); boundary tests (Sol); a confirmed stack that fails the
  gate becomes round 2's candidate base, not a ship (Opus); the early stop enforced on runs
  started past the first four families (Sol); one budget wording and G2's funding stated (Opus);
  gate funded at ~7 h (Sol); per-thread singular counters in the smoke test (Opus). Withdrawn by
  their authors: G out of round 1 (Sol), the depth contradiction (Opus). Still unverified: the
  replacement reading of YaneuraOu's log (Sol; no figure from it is used).
- r4 (2026-11-15), council round 3 (Gemini 9.8, Opus 9.3, Sol 9.2; brief
  `matches/s42/council_r3_brief.md`; no blockers). Applied after the round, not re-scored, each
  the member's own condition for a higher score: runs that finish before the first four families
  have results are provisional, and the early stop is decided first (Sol, "9.5"); the band
  counter runs with D on and off and over the game pilot, and the `corr_learnable` change is
  dropped if it reads 0; A2's kill denominator named and `room == 0` counted; B's re-measurement
  at A1's margin listed as a step (Opus, "9.6").

## 10. Results

### Step 0 (2026-11-15, all passed)

- **Branch** `s42-cand` (worktree `yse-worktrees/s42-cand`, from `main` `6285375` = engine `f3fd207`,
  uncommitted): `SHOGI_MATE_NODES`; switches `SHOGI_S42_SE_DOUBLE` (A2), `SHOGI_S42_SE_SHALLOW` (B),
  `SHOGI_S42_LMR_CHECKS` (E), `SHOGI_S42_EVASION` (F); ProbCut's G11 fix (entry \|beta\| < 29,000, a
  cut with \|v\| ≥ 29,000 returns beta) and the `corr_learnable` range under ProbCut's switch;
  counters `SHOGI_S42_STATS=1` (stderr `S42STATS` after each search). Mac build
  `bins/usi-engine-s42` (md5 85b44a52…); PC build `C:\Users\fgant\yse-s42cand` (magic table md5
  4364196d…, a1 md5 0545dcb6…).
- **Identity**: `search_bench --depth 9`, the 34 corpus positions that pass its filter, work
  fields: fingerprint (TIME-BOUND lines aside), every `qnodes=` and the TOTAL (nodes 616,102,
  all_nodes 2,851,655) identical to `f3fd207`.
- **Tests**: six new (`s42_*`: extension room 2/1/0 and today's rule for every room; ProbCut
  result for ±28,999, ±29,000, 30,000 and mate; `corr_learnable` with ProbCut on and off; B's
  window; F's conditions; E's predicate); `cargo test --lib` 1,171 passed, 0 failed.
- **Determinism**: `f3fd207` vs itself, 10 pairs at 400k: 10/10 identical; `s42-cand` all-off
  (solver 200k) vs `f3fd207`: 10/10 identical **and the same 20 games** as the first pilot.
- **Mac = PC**: `go depth 10` on 8 positions identical with all switches off, with each of the
  six switches alone, and with all six on.
- **Guard reuse**: the installed `f3fd207`, and `s42-cand` with `SHOGI_MATE_NODES=200000`, each
  replay the first pair of S41's `g3ship_16x` on seeds 43800-43809: **20/20 games identical**.
  The S41 row is reused.
- **40-move sequence** (g1108 plies 30-69, 1 thread, 400k, solver 9k): nps all within 2% of the
  baseline's 367.5k (g2 −1.8%, m16 +2.0%), so **every run is at 400k vs 400k**. Completed depth:
  baseline (the shipped stack) **10.80** (G8: the gate 10.9).

| Setting | Depth | Counters (40 moves) | Nodes to d10 (40 pos.) | Kill rule |
|---|---|---|---|---|
| baseline (margin 2) | 10.80 | singular tests 35,636, extended 58.0%, multicut 31.6% | 1.00 | |
| margin 0 | 10.45 | extended 60.8% | 1.13 | reference |
| margin 4 | 11.05 | extended 55.5% | 0.92 | |
| margin 8 | 11.90 | extended 46.7% | 0.74 | |
| **margin 16** | 12.05 | extended 40.0%, multicut 30.0% | 0.65 | **A1** (nearest half of 58%) |
| A2 check + singular two plies | 10.80 | **579 two-ply / 20,758 singular extensions searched = 2.8%**; budget exhausted 33,265 (vs 4,391) | 1.10 | **killed** (< 5%) |
| B shallower table move | 10.72 | 2,779 / 32,852 singular tests = **8.5%** | 0.97 | kept (≥ 2%; re-measured at 16 if A carries) |
| C IIR | 11.05 | 118,846 reductions | 0.87 | kept |
| D ProbCut | 10.82 | 55,896 tries, 34,731 cuts | 1.02 | kept |
| E LMR quiet checks | 10.82 | — | 1.05 | kept |
| F evasion pruning | 10.80 | 74,717 prunes | 1.06 | kept |
| G1 history LMR | 11.05 | — | 0.96 | kept |
| G2 history pruning | 10.80 | — | 1.03 | kept |

- **G11 band counter**: 0 stored and 0 learned scores in [29,000, 30,000) in every setting
  above, ProbCut on included, and over 20 games at 400k with ProbCut on (3.24M tries, 1.67M cuts)
  and off. So the `corr_learnable` change is **dropped from D**: it never fires, so D's screen is
  the same with or without it, and it will not be in a D ship.
- With A2 killed offline, the Mac's queue is E, A1 (margin 16), B, G1, G2, and G2 is funded
  (§5 step 1).

### Round 1

Seed blocks: E 48000, A1 48100, B 48300, G1 48400, G2 48500 (Mac); C 48600, F 48700, D 48800
(PC); Check 0 49000. Scripts `matches/s42/round1/` (`screen.sh`, `pc_screen.sh`,
`hold_pc_screen.sh`).
- PC C (IIR, `SHOGI_LEAN_IIR=1` vs baseline) from 22:55; Mac E (`SHOGI_S42_LMR_CHECKS=1` vs baseline) from 23:16.
- **E** (LMR on late quiet checks, Mac 48000-48011): stage 1 146-13-153, **−8 ± 17** (95% −42 ..
  +27), think ratio 1.001: **stopped at stage 1** (< −5), 00:30. Family E done, no carry. (S41
  measured +25 ± 13 for the same switch alone, before the tactical LMR shipped.)
- Mac A1 (`SHOGI_LEAN_SE_MARGIN=16` vs baseline, 48100-48111) from 00:31.
- **A1** (singular margin 16, Mac 48100-48111): stage 1 168-11-133, +39 ± 17; final **320-27-253,
  +39 ± 12** (95% +15 .. +63), halves +42 ± 18 / +36 ± 17 (z +0.24), think ratio **0.975**
  (within 5%; margin 16 searches a little faster per move): **carried, the first carry**, 02:41.
  Family A done (A2 killed offline).
- **C** (IIR, PC): stage 1 159-14-139, **+22 ± 19**, think 0.989: passed, running to 600.
- B re-measured at margin 16 (step 0 item 5's rule): fires on 11,226 / 41,912 singular tests =
  26.8% (kept); nps −1.2% vs margin 16 alone, so 400k vs 400k. Mac **B as A + B vs A**
  (`SHOGI_LEAN_SE_MARGIN=16,SHOGI_S42_SE_SHALLOW=1` vs `SHOGI_LEAN_SE_MARGIN=16`, 48300-48311)
  from 02:47; provisional until C and F have results (§5 step 1). The early stop can no longer
  fire (A carried).
- Housekeeping: macOS's /tmp cleaner deleted scratchpad files not read for ~3 days (the S41
  drivers, `g1108.moves`, `pos40.tsv`). Rebuilt `g1108.moves` from
  `matches/user_games/2026-11-08_even_loss114.txt` (reproduces the margin 16 counters exactly);
  tools, data and the four binaries copied to `matches/s42/tools/`. The PC holder and the Mac
  waiters are re-armed at the 2 h background limit; the games themselves are not affected (the PC
  script no longer relaunches seeds that are still running on a reconnect).
- **C** (IIR, PC 48600-48611): final **296-25-279, +10 ± 13** (95% −16 .. +36; unrounded +9.85),
  halves +21 ± 19 / −1 ± 18 (z +0.84), think 0.992: **not carried** (the bar is > +10), 02:52.
  Family C done.
- PC **F** (evasion pruning, `SHOGI_S42_EVASION=1` vs baseline, 48700-48711) from 02:54.
- **B** (A + B vs A, Mac 48300-48311): stage 1 131-24-157, **−29 ± 19** (95% −65 .. +7), think
  1.010: **stopped at stage 1**, 04:03. Family B done.
- Mac **G1** (`SHOGI_LEAN_HIST=3` vs baseline, 48400-48411) from 04:05; provisional until F has a
  result.
- **G1** (history-adjusted LMR, Mac 48400-48411): stage 1 178-17-117, +69 ± 18; final **328-27-245,
  +48 ± 13** (95% +23 .. +74), halves +41 ± 19 / +56 ± 18 (z −0.59), think 0.995, 06:16.
  **Provisional** (> +10) until F has a result. G2 is not started: whatever F gives, the second
  carry is decided by F's result (F's own if F > +10, which counts first as one of the four
  families; otherwise G1's), and either ends screening.
- **F** (evasion pruning, PC 48700-48711): stage 1 155-10-147, +9 ± 18; final **294-24-282, +7 ± 13**
  (95% −19 .. +33), halves +1 ± 19 / +13 ± 18, think 1.002: **not carried**, 06:50. Family F done.
- **All four families have results** (E stopped, A carried, C and F not carried), so G1's
  provisional result counts: **G1 is the second carry** (+48 ± 13). Screening ends (G2, D and
  Check 0 not run; D and G2 go to round 2 if it runs).

### Stack confirmation

Stack = singular margin 16 (`SHOGI_LEAN_SE_MARGIN=16`) + history-adjusted LMR (`SHOGI_LEAN_HIST=3`),
vs the baseline (all switches off = `f3fd207`). 40-move sequence at 800k: baseline 386.3k nps,
depth 12.18; stack 376.0k (−2.7%, within 3%: 800k vs 800k), depth 14.00.
- Pilot (24 games, 47800-47811 × 2, 800k): 12-1-11, think ratio **0.987** (within 5%: no
  rescaling).
- **Confirmation** from 07:10: Mac 47900-47906 × 84 (588 games, `round1/confirm.sh conf`), PC
  47907-47911 × 84 (420 games, `round1/pc_confirm.sh`). Rule: > +15 → gate; think ratio within 5%.
- Timing correction: splitting the seeds saves Mac cores, not hours. Each seed still plays 84
  games at 800k (~7.3 h per seed on the Mac, as S41's 1,008 games on 12 seeds), so the Mac share
  ends ~14:30 and the PC share (slower per core) ~17:00, not 4.3 h / 5 h as §2 said.
- **Mac share done** (14:04): 588 games, 330-32-226, +62 ± 14, think 0.980 (for information; the
  rule reads all 1,008).
- Prepared while the PC finishes: ship branch **`s42-ship`** (worktree `yse-worktrees/s42-ship`, from
  `main`; uncommitted): `SHOGI_LEAN_SE_MARGIN` default 2 → 16, `SHOGI_LEAN_HIST` default 1 → 3 (the
  env switches still override), `lean_hist.rs` module comment corrected (G10). None of the failed
  candidates' code, no `SHOGI_MATE_NODES`. Fingerprint identical to the screened stack
  (`s42-cand` + the two env values; all_nodes 2,381,225); with the old values set, identical to
  `f3fd207`. `cargo test --lib` 1,165 passed. Mac binary `tools/bins/usi-engine-ship42` (md5
  a3f03855…); PC `C:\Users\fgant\yse-s42ship`; `go depth 10` on 8 positions identical Mac/PC for
  the ship and for the baseline (`yse-s41ship` = f3fd207's source). Gate scripts
  `matches/s42/gate/` (seeds Mac 4231/4232, PC 4233/4234, 250 per stream, cap 1,000); guard
  `matches/s42/guard/guard.sh` (ship at 1.6M vs YaneuraOu 100k, paired with `g3ship_16x`).
- **PC share done** (17:13): 420 games, 238-21-161, +64 ± 16, think 0.975.
- **Confirmation result: 1,008 games, 568-53-387, 59.0%, +63 ± 11** (95% +42 .. +84), halves
  +59 ± 15 / +67 ± 15 (z −0.37), think ratio **0.978**. **Passes (> +15, ratio within 5%): on to
  the gate.** Mac +62 ± 14, PC +64 ± 16 (same engine, different seeds).

### Gate (s42-ship vs f3fd207)

- 17:15, **void start**: the PC folder `C:\Users\fgant\s42gate` already existed from an older gate
  (2026-11-10, with its `stop` marker and 500 games). The PC job exited at once on the marker, and
  the controller pooled those old games with the Mac's header-only files and logged a "FAIL"
  (LLR −4.24, 500 games, none of them this gate's), stopping the Mac streams before any game
  finished. Files moved to `matches/s42/gate/void_1715/`; no game of this gate was played or
  counted. Scripts repointed to a new, empty `C:\Users\fgant\s42gate2`.
- **Restarted 17:17**: Mac seeds 4231/4232, PC 4233/4234, 250 per stream, pooled SPRT [0, 40] every
  5 min (`gate/gate_ctl.sh`), cap 1,000 (no PASS by then → no ship).
- **Result: FAIL** at 22:48, LLR −3.00 (bounds ±2.94) after **802 games: 398-372-33, 51.6%, +11 ± 12**
  (95% −13 .. +35; a stopped SPRT). Streams: Mac 4231 95-95-8 (0), Mac 4232 100-89-9 (+19), PC 4233
  111-79-9 (+56), PC 4234 92-109-7 (−28); their spread is ordinary for ~200 games each (χ² 6.5 on 3
  df, p ≈ 0.09). No time losses (770 games ended by mate, 23 repetition, 10 move limit). The user's
  GUI engine was running on the Mac at the end, so a user game may have overlapped part of the
  Mac streams; it slows both sides alike, and the Mac streams do not stand out.
- **Per the rules: not shipped; no YaneuraOu guard.** The confirmed stack (margin 16 + history LMR)
  is recorded as round 2's candidate base (§5 step 2): round 2 may screen new families on top of
  it, and the combined stack gets one confirmation and one gate against `f3fd207`.

### Reading (lead, for the council)

- **The screen-to-gate transfer failed for this stack**: 1-thread fixed nodes +63 ± 11 at 800k
  (+39 and +48 at 400k for the parts) against +11 ± 12 at the gate, a gap of ~52 ± 16 (3.2 SE).
  In S41 the same pipeline gave +74 ± 10 → +72 ± 28. So the difference is not the screen's
  machinery (calibrated, deterministic, Mac = PC); it is something about these two changes at 1 s
  and 4 threads.
- Candidates, each testable offline or cheaply, none checked yet:
  (1) **history-adjusted LMR at 4 threads**: the reduction reads per-thread history; at the gate
  each of four threads sees about a quarter of the nodes per move, so its history is thinner and
  the reductions differ from the 1-thread screen's (the council flagged this mismatch for family G
  in round 1; G10 only showed the tables survive across moves, not that they fill alike).
  (2) **singular margin 16 and the shared table**: at 4 threads table entries come from helpers
  too, which changes how often the singular test and the multicut run (Opus round 2 asked for
  per-thread singular counters in the smoke test; the smoke test of step 0 item 6 was not run).
  (3) **depth regime**: the gate's 4-thread search is ~1-2 plies shallower than the 800k
  confirmation; S41 showed self-play gains shrinking with depth in the other direction, so this
  is the weakest candidate.
- The cheapest discriminating test: the gate's conditions for one component at a time, or a
  4-thread fixed-node match (e.g. 4 threads × 400k total) for each part, which separates (1)/(2)
  from time effects.
- **Missed step**: step 0 item 6 (4-thread smoke test with per-thread singular counters) was not
  run before round 1; it would not have predicted the gate result, but the per-thread counters
  are exactly the data needed for (1) and (2).

### Diagnostic (2026-11-17): why the stack lost its gain at the gate

Council (Opus, Sol, Gemini), five rounds (RESEARCH_S40 "Council (2026-11-17 ...)"; briefs
`matches/s42/council_r4_brief.md` .. `r8`). User: "Go ahead and test the changes or council first."

**Offline** (no games; `matches/s42/diag/`):
- Counters added to `s42-cand` (uncommitted; off unless `SHOGI_S42_STATS=1`; search unchanged
  with them off, 12 moves at 1-thread 400k identical): per thread (lean_inner entries, singular
  tests / extensions / multicuts, history-LMR adjustment up / down / clamped), helper overrides
  and changed moves, `GLOBAL_SEARCH_NODES` after the helpers join; switch
  `SHOGI_S42_NO_OVERRIDE`. Mac binary `tools/bins/usi-engine-s42b` (md5 881dc3c6…).
- 40-move sequence at 1 s (`diag/t1sb_*`, `t1sc_*`), base / margin 16 / G1 / stack:
  nodes per move 276-293k on 1 thread, 992k-1,078k on 4 (7.4-8.3M lean_inner entries per thread
  over 40 moves vs 8.4-8.9M on 1 thread: per-thread work at the gate ≈ 1 thread at 1 s).
  Overrides 6-16 of 40, a different move 0-2 of 40, not more with the stack. Singular extension
  rate 58-60% (margin 2) / 40% (16) on 1 thread, 53-55% / 35-37% on 4; multicut 27-31% / 30-36%;
  the 1→4-thread shift is the same for both margins. History LMR: reduction raised on 41-46%,
  lowered on 6-8%, clamp changes it on 25-34%, on 1 and 4 threads alike. Time after the last
  completed iteration: 1T base 22%, stack 20%; 4T base 37% / 27%, stack 27% / 21%.
- Gate by halves: Mac first halves +46 ± 24, second halves −26 ± 21, PC +12 ± 15 (level within
  streams; streams +56 and −28 on different openings). All 802: +11 ± 11 (primary). The GUI
  engine process (alive since 11-15 14:40) had used 17.5 CPU-hours; a GUI game during the gate is
  plausible, untimed. Gate reasons: 770 no legal moves, 23 repetition, 10 move limit, 0 time.
- Load probe (1 thread, 1 s): 12 processes −5.3% nps, 8 processes −3.8%.
- Code findings, not part of this diagnostic (round 2 candidates): `moves_equal`
  (`search_engine.rs:9678`) ignores promotion; ply-0 continuation history is empty
  (`lean_hist.rs:189-192`); `go nodes` forces one thread (`lib.rs:582`).

**Runs** (started 2026-11-17 06:38):
- **D1** (Mac, `diag/d1.sh`): the gate's binaries, 1 thread, 1 s, 8 seeds × 76 (49100-49107),
  stage 1 at 320 (< −5 stops). Read as D1 − gate (+11 ± 11): ≥ +30 → round 2 adds a 4-thread
  confirmation before any gate; D1 < +20 → round 2 confirms at ~280k nodes; else no change.
  +8 .. +32 at 608 → extend to 1,008 (`TAG=d1x`).
- **G1 alone at the gate's conditions** (PC, `diag/pc/pc_g1.sh`, `C:\Users\fgant\s42diag`):
  `s42-cand` + `SHOGI_LEAN_HIST=3` vs `s42-cand` all off, 4 threads, 1 s, 2 × 300, seeds
  49200/49201, stage 1 at 320.
- Then **margin 16 alone**, same conditions, Mac + PC. A component ≥ +20 qualifies as round 2's
  base (both: the higher, margin 16 on a tie); +10 .. +20 "unresolved"; neither → round 2 from
  `f3fd207`. Nothing ships from this diagnostic.

**D1 result** (2026-11-17 08:47, complete, 608 games): stack vs f3fd207 at 1 thread on the
clock **+51 ± 13** (95% +26 .. +76), seed halves +51 / +51, think ratio 0.985, no time losses, GUI
idle throughout. D1 − gate = +40 ± 17 (≥ +30): round 2 adds a 4-thread confirmation before any
gate. The stack keeps its gain on one thread at 1 s; the loss needs 4 threads. No extension.

**Change (2026-11-17, lead; user gave both machines for the day and asked for shippable
progress):** the two single-component runs are monitored as gates in the gate's own format (two
seeds pooled, 4 threads, 1 s, time-loss rule, SPRT [0, 40], `scripts/sprt.py`, every 5 min by
`diag/ctl.sh`; a decision stops the run). Margin 16 alone runs on the Mac only
(`diag/m16/m16_mac.sh`, `usi-engine-s42` + `SHOGI_LEAN_SE_MARGIN=16` vs all off, seeds
49300/49301, 2 × 300, stage 1 at 320), from 08:47. A PASS goes to the YaneuraOu guard
(`guard/guard.sh` with that component's ship binary) and then to the user. Not reviewed by the
council; the G1 run's first 128 games (+52 ± 29) had been seen before it was made a gate. Ship
binaries with the one default compiled in: `tools/bins/usi-engine-g1ship` (`lean_hist` default 3,
md5 0be7e397…) and `usi-engine-m16ship` (margin 16, md5 5ba859b4…), from 6285375 in worktrees
`s42-g1` / `s42-m16`; each matches its env run at 1 thread, 300k nodes (same depth, score, nodes,
move).

**Component gates (2026-11-17):**
- **History LMR alone, PASS** (PC, 11:02): 316 games, LLR +3.03 [−2.94, 2.94]; **+43 ± 18**
  (95% +8 .. +78), halves +61 ± 24 / +25 ± 26, think ratio 0.990, no time losses, 4 engines on
  the PC throughout. Expect the true value below +43 (a passing SPRT overstates).
- **Margin 16 alone, FAIL** (Mac, 10:52): 146 games, LLR −3.00; **−26 ± 24** (95% −73 .. +22),
  think ratio 1.007. At 1 thread margin 16 was +39 ± 12 at 400k nodes. So the stack lost its gain at
  4 threads through margin 16: singular-extension changes must be screened at 4 threads.
- YaneuraOu guard for `usi-engine-g1ship` (`guard/guard_g1.sh`, rule as `guard.sh`: g1ship −
  f3fd207 < −29 → no ship) started 10:52 on the Mac. Ship needs its result and the user's OK.
- PC, from 11:10: screen of history pruning on top of history LMR (row G2 of the round 1 list,
  never run): `SHOGI_LEAN_HIST=7` vs `=3`, round 1's screen (`round1/pc_screen.sh`, 1 thread,
  400k nodes, 12 seeds × 50 from 49400, stage 1 at 26 per seed). History transferred from
  1-thread screens to the gate (+48 → +43), so this screen is read like round 1's (> +10 carries).
- **YaneuraOu guard for `usi-engine-g1ship`: passed** (12:37). 240 games at 1.6M vs YaneuraOu
  halfkpvm 100k, paired with S41's f3fd207 row: g1ship 46.2% (−26), f3fd207 42.7% (−51);
  difference **+25 ± 32** (95% −38 .. +87), above the −29 no-ship line. Waiting for the user's OK.
- Mac, from 12:40: screen of ProbCut (row D, with the G11 fix; never run) on top of history LMR:
  `SHOGI_LEAN_HIST=3,SHOGI_LEAN_PROBCUT=1` vs `SHOGI_LEAN_HIST=3`, round 1's screen
  (`round1/screen.sh`, tag pc3, seeds 49500-49511). ProbCut reads the table; if it carries, it
  is checked at 4 threads before a gate.
- **Shipped** (user's OK, 2026-11-17): `ada63e6` "Search: history-adjusted LMR on by default"
  (`lean_hist.rs` default 1 → 3; code identical to the tested `s42-g1` build), and
  `engines/usi-engine-stable` = `usi-engine-g1ship` (md5 0be7e397…). The baseline for the next
  gates is `ada63e6`. Not pushed.
- ProbCut on top of history LMR (Mac, pc3): **stopped at stage 1, −7 ± 19** (312 games). Stays off.

### After the ship (2026-11-17 afternoon, baseline `ada63e6`)

- History pruning (`SHOGI_LEAN_HIST=7` vs 3, PC): **+2 ± 13** (600 games). Not carried.
- Council (direction, RESEARCH_S40 last section). Runs started with the user's go-ahead:
  divisor screens `lmr_hist_div` 12200 (Mac, tag dv12, seeds 49600-49611, from 15:17) and 5700
  (PC, tag dv57, seeds 49700-49711), each vs 8123, 1 thread, 400k nodes, 600 games, > +10 carries.
- **Promotion identity fix** (worktree `s43` from `ada63e6`, uncommitted): the singular test's
  exclusion and the table-move test in `lean_inner` (`lean.rs:705, 711`) use `lean_hist::same`
  (promotion included) instead of `moves_equal`; switch `SHOGI_LEAN_PROMO_SAME` (default on, 0 =
  old). Unit test `singular_identity_sees_promotion`. Binary `tools/bins/usi-engine-s43p` (md5
  eb77eb83…). Identity check (`diag/s43/ident.sh`, 40 positions at 200k nodes, 1 thread): switch
  off = byte-identical to `usi-engine-g1ship`; on, every position differs (from the first).
- **Speed change tried and taken out:** one hand copy per node with apply/undo instead of a copy per
  move (search identical) gave +0.4% nps on 1 thread (313.5k/315.9k vs 313.9k/317.7k), within
  noise. The 8.9% malloc in S41's profile is not those copies.
