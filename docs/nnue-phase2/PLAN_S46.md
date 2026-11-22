# PLAN S46: repair the search, then decide on a new core (r2, after council, 2026-11-21)

Status: **§5 steps 1-3 approved; step 1 (the delta fix) shipped 2026-11-22, §10.** The user approved
the design and an Opus / Sol / Gemini council (2026-11-21). Step 4 (the core) needs a separate OK.

**The change from r1:** the council found a defect worth more than any feature tried since the
S34-S35 rewrites (§1.4). Repairs now come first; the new core is decided against the repaired engine.

The user's framing: "I think you are doing small things that buy us little. We need to consider
something bigger."

## 1. What is measured

### 1.1 The gap (PLAN_S45 §15, PLAN_S44 §4)

Ours against YaneuraOu V9, both on halfkpvm, 1 thread, fixed nodes:

| Their nodes | Our nodes | Games | Our score |
|---|---|---|---|
| 1k | 1x | 240 | 40.8% |
| 3k | 1x | 240 | 11.7% |
| 10k | 1x | 240 | 5.2% |
| 30k | 1x | 240 | 3.3% |
| 100k | 1x | 120 | 0.8% |
| 10k | 4x | 240 + 480 (two opening sets) | 27.3%, 30.9% |
| 10k | 8x | 240 | 48.8% |
| 100k | 16x | 240 | 50.2% |

So parity costs about 8x at 10k and 16x at 100k. One doubling of our nodes is worth ~160 Elo on
the 10k row (27.3% -> 48.8%) and ~140 at 100k (PLAN_S41 §7). In doublings against the user's PC
opponent: search ~4 at 100k, speed ~2 (4.1x nps), network ~1.

Limits: one opponent, 1 thread, at most 1.6M nodes a move. The user plays 16 threads at 100M+
nodes a move. Nothing here says what the ratio is there.

### 1.2 Part of the gap is budget thrown away (`matches/s46/probe/waste.py`)

Under `go nodes N` the move comes from the last reported iteration; the interrupted one is
discarded. Share of N that move comes from (40 positions, halfkpvm):

| N | 1k | 3k | 10k | 30k | 100k | 400k |
|---|---|---|---|---|---|---|
| mean | 49% | 64% | 64% | 62% | 70% | 73% |

That is at most ~0.5 doubling at 100k (an upper figure: YaneuraOu's own last iteration is also
only partly useful, and a discarded iteration still fills the table). On a clock the engine also
stops starting iterations late (RESEARCH_S40 §5a: move from 562 ms of 1,000). Against this,
RESEARCH_S40 §5b: a narrow partial-iteration rule gave +21% effective time and **gated 0 ± 35**.
That rule adopted a move only if the score was inside the window, so never a fail-high.

The testbed also flatters us a little: `go nodes` gives our root mate solver up to 200k df-pn
nodes a move that are not counted (PLAN_S41 F16).

### 1.3 The first ten switches (the eleventh mattered, §1.4) (`matches/s46/probe/arms.sh`, 40k vs 10k, halfkpvm)

First pass, 240 games an arm, the same openings (baseline 27.3%, SE of a difference ~4 points):

| Arm | Score |
|---|---|
| singular extension off | 30.8% |
| check extensions off | 31.5% |
| quiescence checks off | 32.5% |
| quiescence evasion pruning on | 37.9% |
| superior/inferior hand rule off | 29.2% |
| improving flag on (7) | 36.5% |
| margins x0.6 | 32.1% |
| margins x1.6 | 25.4% |
| correction history off | 26.2% |

Replication on fresh openings, 480 games an arm, paired by opening (baseline 30.9%):

| Arm | Difference in score | Elo |
|---|---|---|
| quiescence evasion pruning | −0.9 ± 3.0 | −8 |
| improving flag | −0.2 ± 3.0 | −2 |
| **both together** | **+7.1 ± 3.1 (z 2.3)** | **+55** |

- The two best first-pass arms were winner's curse. The first baseline was on the low side.
- Features this engine has carried for weeks (singular extension, check extensions, quiescence
  checks, correction history) are each worth no more than noise against YaneuraOu on this rung
  (95% about ±70 Elo). In self-play some screened strongly (singular extension +66 ± 13).
- The combination is one unreplicated z 2.3. It is a hint that features interact, not a result.
- **Rung resolution:** 480 games (6-7 minutes) resolve ~±45 Elo at 95%. ±20 needs ~2,400 games
  an arm (~30 minutes). It judges milestones quickly, not small features.

### 1.4 The council's kill test: quiescence delta pruning is broken

`lean.rs:1636` skips a capture when stand-pat + board value + 200 <= alpha. A capture in shogi also
puts the piece in hand, so it swings about twice the board value (Opus, 3,358 captures on
halfkpvm: the static gain exceeds the bound in 49%; median bishop gain 1,954 against a bound of
1,000). Present since d29c658 (S34); no game test of it found in the notes or `matches/`.

`SHOGI_LEAN_OFF=q_delta` against the same engine:

| Conditions | Games an arm | Before | After | Elo |
|---|---|---|---|---|
| 40k vs YaneuraOu 10k, halfkpvm, seeds 47000 | 480 | 30.9% | 58.4% | **+199** (z 9.4) |
| same, seeds 48000 | 480 | 34.8% | 59.8% | **+178** (z 8.3) |
| same, ours stable + a1, seeds 48000 | 480 | 43.8% | 53.5% | **+68** (z 3.1) |
| **self-play, stable + a1, 1 thread, 400k each, seeds 49000** | 240 | | **59.2%** (137-10-93) | **+64 ± 21 SE** (95% +23 .. +105) |

- Not measured: 4 and 16 threads, clocks, equal time, 100k+ against YaneuraOu, the swing-correct
  margin in place of switching the rule off.
- The bug hurts halfkpvm about three times as much as a1. With it, a1 led halfkpvm in our search
  (43.8% vs 34.8%, same openings); without it halfkpvm leads (59.8% vs 53.5%). **PLAN_S44 §4's
  "a1 = halfkpvm, the network is not the lever" was measured through this bug and must be re-run.**
- Same test, `SHOGI_LEAN_CORR_W=2,2,3,56`: −3 Elo (z −0.1); with q_delta +181.
- Uncounted root mate search at the 1k row: switching it off −21 Elo (z −1.0), so that row stands.

### 1.5 Already tried (do not assume these pay)

| Item | Result | Where |
|---|---|---|
| Staged generation / table move first | no gain twice, −14 ± 18 | S34, S35, PLAN_S45 §14 |
| Partial iterations (narrow rule) | 0 ± 35 at the gate | RESEARCH_S40 §5b |
| ProbCut, IIR, evasion pruning alone | +0 to +10 each | PLAN_S42 round 1 |
| History pruning | +2 ± 13 | PLAN_S42 |
| Mate-in-one in search / quiescence | no gain / −60 | PLAN_S45 §13.1, S34 |
| Singular margin 16 | +39 on 1 thread, −26 on 4 | PLAN_S42 |
| SPSA over 24 constants (20,000 pairs, ~80 ms) | small moves; stack passed | RESEARCH_S40 |
| Lean search rewrite (S34) | four-piece rung 57% -> 90% | SESSION_LOG_034 |
| History rewrite (S35) | 10k rung, same network, 41% -> 74% | SESSION_LOG_035 |

The two largest search gains this engine has had were the two rewrites.

## 2. What the search does today, against published practice

Read from 50d3ac4. **"Read" means seen in the code, not shown to cost strength.**

**Root and iteration driver** (`search_engine.rs`)
- R1. The root is the old `search_at_depth` (`:6483`) with the lean search below it: its own PVS,
  ordering, reductions and store.
- R2. Root moves are re-ordered every iteration by the table move, then ply-0 history (`:6651`).
  The previous iteration's scores and effort are not kept.
- R3. An interrupted iteration is discarded (`:16759`). A fail-high move is not adopted
  (`:16828`), so a better move proven by a fail-high is lost if the re-search times out.
- R4. A fail-high re-search keeps the full depth. Window growth restarts from a fixed size with no
  use of score stability.
- R5. No time extension on instability. No new iteration after a share of the time (`:16200`).

**Transposition table**
- T1. Direct-mapped, one entry a slot (`thread_safe_table.rs:422`, `:641`); no buckets.
- T2. An entry holds no static evaluation and no "was PV" bit. The table score is never used as a
  better evaluation for pruning.
- T3. A fail-low node writes the first ordered move as its best move (`lean.rs:1027`). With a
  table move that is the table move, so only nodes without one get an invented move (council
  correction); IIR then rarely sees those again.
- T4. Quiescence does not use the main table. It has a per-thread direct-mapped cache with no move
  and no evaluation (`lean.rs:1542`).

**Node logic** (`lean.rs`)
- N1. Shallow pruning is gated by remaining depth, not by the depth after reduction (`:759` and
  around), so late moves at depth 9+ are never pruned.
- N2. "Improving" is off by default and then reads as true everywhere (`:399`).
- N3. Reductions do not know: was-PV, table move is a capture, how many children cut, improving.
  Quiet checks are never reduced (`:839-856`).
- N4. Re-search after a reduced fail-high is always at exactly full new depth.
- N5. Extensions: +1 for every non-losing check and for a singular move, capped only at the
  iteration depth per path (`:797`). No depth-aware gate, no negative extension. 48% of singular
  tests extend (PLAN_S45 §13.2), and singular extension is most of our extra nodes per depth.
- N6. History learns only at a cutoff, only on this node's moves. Nothing for the previous move
  when this node fails low, at table cutoffs, or after a reduced re-search. No evasion or check
  statistics. Two continuation distances.
- N7. `lmr_tactical_candidate` computes SEE on the board after the move (`:839`; known since S44).
  Almost always the same value (council, by reading): not worth work.
- N8. The full legal list is generated, scored, sorted and cloned at every node (`:606`).

**Quiescence** (`lean.rs`)
- Q1. In check every evasion is searched, ordered least valuable piece first (`:1603`). In shogi
  that puts every interposing pawn drop first and prunes none.
- Q2. At ply 0 all non-losing checks, drops included (`:1539`). All non-capturing promotions at
  every ply.
- Q3. A hard cap of 8 plies (`:1256`) that returns the static evaluation.
- Q4. No table move, no history in ordering.

**Defects found by the council, each checked by the lead in the code**

| | Defect | Where | Evidence of cost |
|---|---|---|---|
| D1 | Correction update has loop gain depth/4: overshoots from depth 5, unstable from 8 | `lean_corr.rs:183` | Replayed: worse than no correction with noise. Rung: none (−3) |
| D2 | Delta pruning uses half the capture swing | `lean.rs:1636` | **+64 to +199, §1.4** |
| D3 | SEE pruning of non-capturing promotions never fires (SEE returns 0) | `lean.rs:784`, `:1116` | unmeasured |
| D4 | Perpetual-check results are stored as true mates and can end the iteration loop | `lean.rs:1040`, `search_engine.rs:17119` | unmeasured |
| D5 | Quiescence evasions search king moves last, after every interposing drop | `lean.rs:1603` | unmeasured |
| D6 | Within a move a table slot refuses a newer entry of equal depth and bound | `thread_safe_table.rs:673` | unmeasured; needs a saturated table |
| D7a | Aspiration window is always ±200 | `iterative_deepening.rs`, `types/search.rs:1749` | unmeasured |
| D7b | Root: no check extension, no history update | `search_engine.rs:6483` ff. | unmeasured |
| D7c | Useless non-promotion twins of pawn, bishop and rook generated everywhere | `moves.rs:642` | unmeasured |
| D8 | Quiescence cache reuses results found with checks at nodes searched without (Sol) | `lean.rs:1539-1548` | unmeasured; may be harmless |

**Scale.** Margins are centipawns tuned on a1, whose scores are ~0.57x halfkpvm's (PLAN_S45
§13.2). Any diagnostic on halfkpvm runs them at the wrong scale.

## 3. Field evidence, and how far it transfers

| Claim | Source | Transfer |
|---|---|---|
| An overhauled shogi search was weaker until tuned as a whole; tuned, R50 at 1 s, R70 at 2 s, R90 at 4-8 s | YaneuraOu V9 post, 2025-08 | High. The same game and the same method; a much stronger starting engine |
| Chess margins and reductions failed in shogi until SPSA | same | High |
| Whole-search SPSA ~R50 | YaneuraOu SPSA post, 2025-07 | Medium; game counts not given |
| Pruning heuristics overlap; removing one lets others catch the same moves, so single-feature Elo misleads | Stockfish community guide (rentry sf_wisdom) | Medium (chess); agrees with §1.3 |
| Continuation history ~63 Elo, fail-soft ~30, main history ~11 | same | Low; no conditions given |
| Flat depth thresholds scale worse to long time controls than proportional ones | same | Medium; bears on N1 and the user's clock |
| Correction history gains more at longer time controls | Chessprogramming wiki | Medium |
| Nothing published on drops, interpositions or shogi quiescence was found | searched 2026-11-21 | This part will be our own |

Ideas only. No GPL source is opened while writing the core, and no engine's constants are used.
Starting values are our own `lean_params` or derived from our own statistics, then tuned.

## 4. The design

A new module `src/search/core/`, selected by a switch (`SearchCore`), with lean untouched as the
control. It owns everything from iterative deepening down:

| File | Holds |
|---|---|
| `stack.rs` | one per-thread array by ply: move, key, static eval, in check, was-PV, cut count, excluded move. Replaces the thread-locals and `Vec` paths |
| `table.rs` | buckets; entry with key, move, score, eval, depth, bound, was-PV, generation; a fail-low keeps the old move; shared by main and quiescence |
| `picker.rs` | stages: table move, good captures, quiets, bad captures; own orders for evasions (captures, king moves, interpositions by history and SEE) |
| `stats.rs` | one update rule for main, continuation, capture and evasion history and correction; used by ordering, reductions and pruning |
| `node.rs` | PVS with three node types; eval refined by the table bound; reductions as one sum of terms in fractional plies; shallow pruning by reduced depth; singular extension with negative extension and multicut; depth-aware check handling |
| `qsearch.rs` | table-backed; stand-pat stored; evasion pruning once not mated; drop-interposition limits; checks as a tuned choice; no fixed ply cap |
| `root.rs` | root-move records (score, previous score, effort, PV); stable ordering; aspiration; adopts fail-highs and completed partial results; threads voted |
| `params.rs` | every constant, in units of a per-network score scale, exposed for SPSA |

Reused unchanged: board and make/unmake, move generation and legality, NNUE, the mate solver,
repetition and hand-rule definitions, USI, `searchmoves`.

Not in scope: network, board speed, time allocation across a game.

Shogi rules that must hold from the first build, each with a test that fails without it:
perpetual check, superior/inferior repetition without table poisoning, pawn-drop mate, no-legal-
move loss, mate distances through the table, `searchmoves`, stop latency.

## 5. The plan (r2), for the user's approval

"The rung" = halfkpvm in both, 1 thread, fixed nodes, against YaneuraOu V9. Every result is also
taken on a1, the network that ships.

| Step | Work | Machine | Rule |
|---|---|---|---|
| **1. Ship the delta fix** | a swing-correct margin vs the rule switched off (one small change); then the 4-thread, 1 s gate, SPRT [0, 40], vs stable | Mac ~1 h of screens; PC ~3-6 h gate | ship only on a gate pass and the user's OK. My pass odds ~75% |
| **2. Repair bundle** | D1, D3, D4, D5, D6, D7, D8, each behind a switch with a test that fails without it; unit model board + hand; discarded iteration and fail-high adoption | 2-3 days of my work; rungs of 10-30 min; D6 at a saturating hash; one gate | bundle gated as a whole. A part joins if it is not worse on the rung and in self-play |
| **3. Re-measure** | 100k x8 and x16 on shared openings, halfkpvm and a1; a1 vs halfkpvm again | Mac ~4 h in pieces, or the PC if it can host the rung | at 100k x8: **>= 45%: keep lean** and tune it; **< 45%: step 4** |
| **4. New core, staged** | root and table first (option C), then nodes and quiescence (§4) with a fresh go-ahead | as below | see below |

If step 4 runs, the council's corrections to r1 apply:
- Root and table are judged also at a saturating hash, not only 40k nodes.
- The untuned interior may be down to −80 on the rung after a coarse tune (r1 said −30 untuned).
- Kill rule at 100k x8 over 480 games, at equal time: >= 50% continue; 45-50% extend to 960;
  < 45% stop.
- SPSA: 8-20 active parameters a phase, self-play on a1, coarse at 40k nodes, then depth-sensitive
  constants at 200-300k. A 4-thread, 1 s pilot first. Judged on both networks.
- The 16-thread guard needs a stated bound: 100 games have SE ~35-40.
- A/A uses fresh openings: the engines are deterministic at fixed nodes.

## 6. The four questions, for steps 1-3

1. **Effect against resolution.** Step 1: +64 ± 21 on the screen; the gate passes a true +40 90% of
   the time. Screens have overstated before (+63 -> +11 in S42), hence 75%.
2. **State as games use it.** The delta fix keeps no state. D1, D4 and D6 do: whole games in one
   process, 4 threads, before the gate.
3. **Test against use.** The gate is 4 threads and 1 s; the user plays 16 threads and minutes a
   move. The bundle gets a 16-thread check before it ships.
4. **The measured gap.** Yes. How much of the 16x the repairs close is unknown until step 3.
   Odds of 8x from repairs alone: ~25% (guess). From repairs and then a core: ~35% (Opus).

## 7. Cost

- Step 1: half a day, one PC gate.
- Step 2: 2-3 days, one PC gate.
- Step 3: half a day of Mac pieces, never during the user's games.
- Step 4, if it runs: 1-2 weeks and 2-3 PC nights, asked for separately.

## 8. Alternatives

- **A. The new core now** (r1). Not chosen: it would hide which repair paid and tune around broken
  units. It stays the plan if step 3 says so.
- **B. Repair lean, in one bundle** (chosen first).
- **C. Root and table first** (the first part of step 4).
- **D. Speed rewrite**: ~1-2 doublings at most, weeks. Later.
- **E. Network**: reopened by §1.4; decided after step 3.

## 9. Council (2026-11-21, direction + plan check)

Members: Claude Opus 5.5 (high) 5/10; GPT-6.2 Sol (high) 6/10; Gemini 4 Argon: no review (Cursor
usage limit until 2026-12-11; not replaced). One round. The r2 plan has not been re-reviewed.

- **Agreed:** the gap is real; don't build the interior first; r1's 50% odds were too high (both
  25%); the kill rule needs an extension band; SPSA at 40k nodes over 40 parameters is wrong;
  judge on both networks; staged generation and thread voting can wait.
- **Differed:** Sol wanted root and table first; Opus wanted to repair and falsify first. The kill
  test decided for Opus.
- **Adopted:** Opus's D1-D7 and kill test, corrections to T3 and N7; Sol's D8, stop-rule band,
  pilot before SPSA, replication of the combination before crediting it.
- **Did not hold up:**
  - `waste.py` inflated by heartbeat lines (both): `waste2.py` found none, same 64 / 70 / 73%.
  - The diagnostic network may not load (Sol): it prints "loaded", and 20 of 20 fingerprints differ.
  - The delta error "mostly cancels on a1" (Opus): +68 on the rung, +64 in self-play.
  - Correction instability explains lost strength (Opus): real arithmetic, −3 on the rung.
  - The lead's r1 claim that no single switch is a lever: q_delta was never among the ten.
- **Left open:** the halfkpvm port compared with YaneuraOu's static evaluation on 1,000 positions
  (its binary here has no working `eval` command); the openings and hash of §1.1's rows differ.

## 10. Results

### Step 1: the delta fix (worktree `s46fix`, 50d3ac4 + `matches/s46/fix/delta600.patch`, 2026-11-21/22)

- `q_delta_bound`: board value + hand value + promotion gain (at least 300) + `q_delta_margin` (600).
  `SHOGI_Q_DELTA_BOARD_ONLY=1` = the old bound and margin: 0 of 40 fingerprints differ from the
  shipped search, Mac and PC. Three tests (the quiescence one fails with the switch on); 1178 unit
  tests pass on the Mac; PC 1179 + the known `test_memory_breakdown`. Same fix fingerprint on both
  machines. About 6% faster per node (20 positions x 1M nodes, two rounds).
- Margin, 40k vs YaneuraOu 10k, 480 games an arm, seeds 48000:

  | Network | old | rule off | swing + 200 | swing + 600 |
  |---|---|---|---|---|
  | a1 | 43.8% | 53.5% | 57.3% | 56.7% |
  | halfkpvm | 34.8% | 59.8% | 47.3% | 62.1% |

- **Confirmation screen** (fix vs stable, a1, 1 thread, 400k nodes, seeds 51000-51011):
  **193-14-105 of 312, 64.1%, +101 ± 20 SE** (95% +62 .. +140), halves 64.7% / 63.5%. Carries to
  the gate (> +10).
- **Gate started 2026-11-22 10:18** (the user gave the PC): `matches/s46/gate/`, PC
  `yse-s46delta`, `s46gate`, seeds 52000 / 52001. The control wrapper was smoke-tested: against
  the shipped build its colour-swapped games are the same game.
- Harness fixed before the first game (engine untouched): decisions need 20 pairs (the final-LLR
  check skipped the minimum), no stream restarts after `stop` / `paused`, running matches are
  detected from Windows, and the Mac side runs in its own session (`ctl.log`, `watch.sh`).
- **Gate PASS 2026-11-22 10:53: 29-2-9 of 40 (75.0%), LLR +4.06** [-2.94, 2.94], no time losses,
  streams 17-0-3 and 12-2-6 (`matches/s46/gate/final/`; a 41st game finished after the cut). Raw
  +191 (95% +100 .. +315) is biased up by stopping early: quote the screen's +101 for the size.
  Read-only disk scans ran at low priority from about 10:36: before them 13-2-5, during 16-0-4,
  move times unchanged (724/749 and 711/742 ms).
- **Shipped 2026-11-22** on the user's OK. Same 40 fingerprints as the gated binary; switched to the
  old bound, 0 of 40 differ from the previous stable. 1178 unit tests pass. The integration suite
  has ~37 failures with or without the fix (three timing tests differed under load and pass alone).

### Step 2, first repairs (worktree `s46repair`, the fix + `matches/s46/repair/r1.patch`, 2026-11-22)

D5 (king moves first among quiescence evasions, `SHOGI_Q_KING_FIRST`) and D3 (SEE pruning sees
non-capturing promotions, `SHOGI_SEE_PROMO`). A test each, failing with the switch off; 1180 unit
tests pass; both off = the gated fix (0 of 40 fingerprints differ). a1, 40k vs YaneuraOu 10k,
480 games an arm, seeds 53000, paired against the fix alone (58.5%):

| Arm | Score | Elo against the fix |
|---|---|---|
| D5 | 52.9% | −40 (z −1.8) |
| D3 | 58.4% | −1 (z −0.0) |
| both | 58.6% | +1 (z +0.0) |

No gain from either. D5's order is right on paper and measured worse or neutral: it stays out
unless a larger run says otherwise. D3 is neutral and may join the bundle as a correctness fix
after self-play. Remaining: D1 at depth, D4, D6 (saturated table), D7, D8, interrupted iterations.

### Correction: the user's games were 16 threads against 4 (2026-11-22)

The user never set engine options, and the GUI sends none: YaneuraOu plays on its default 4 threads,
ours on all logical CPUs (16 on both machines). "The user plays 16 threads" here, in PLAN_S44 and in
the user-game notes is wrong for the opponent (it came from a misread GUI default).

- PC, move 41 of the 2026-11-22 game: ours 16T 4.02M nps, YaneuraOu 4T 5.73M (1.4x), 16T 14.49M (3.6x).
  That game was at about equal nodes, as ours thought 1.5x as long.
- So the speed share of the gap in the user's games is about 0.5 doubling, not 2: those losses are
  search and network. At equal threads the user's games would be harder than the record shows.
- Clock readings and think times in earlier GUI game notes are unreliable: the GUI's clock counted
  timer ticks, and engines got no move history (fixed in ysu 5b68290; 15df20c makes a resignation
  end the game).
