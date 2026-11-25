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

### Network size and step 3 re-measure (Mac, 2026-11-22 evening, `matches/s46/remeasure/`)

**Network, same search.** YaneuraOu V9 + Suisho5 (FV_SCALE 24) against YaneuraOu V9 + halfkpvm, 1 thread,
equal nodes, 240 games a row, seeds 57000-57011 (`netcmp.py`):

| Nodes | Suisho5's score | Elo |
|---|---|---|
| 10k | 51.0% | +7 ± 22 |
| 100k | 51.7% | +12 ± 22 |
| 1M | 55.8% | +41 ± 23 |

The network's share grows with depth. At the user's 10-100M nodes a move it is probably larger again (not
measured).

**Step 3, ours 800k against YaneuraOu halfkpvm 100k** (7b8b1a0, 1 thread, seeds 58000-58011, `rung.py`):

| Our network | Score | Elo |
|---|---|---|
| halfkpvm (yo-diagnostic build of 7b8b1a0) | 46.0% (105-11-124) | −28 ± 22 |
| a1 (shipped) | 40.8% (95-6-139) | −64 ± 23 |

- Same network: parity at 100k now needs about 9x (was 16x before the delta fix, §1.1): ~0.8 doubling
  closed, ~3.2 left. 46% clears §5's 45% line by less than its noise.
- a1 is ~36 Elo below halfkpvm in our search at 800k (§1.4 had ~+40 at 40k). The shipped engine needs
  about 11x.
- In doublings against YaneuraOu + Suisho5 on equal hardware: search ~3.2; network ~0.5 at 1M (a1 to
  halfkpvm ~36, halfkpvm to Suisho5 ~41), likely more deeper; speed ~1.8 (3.6x nps at 16 threads).

### Time manager (worktree `s46tm`, 7b8b1a0 + `matches/s46/tm/tm1.patch`, 2026-11-22)

Found in the Mac game of 2026-11-22 (`matches/user_games/2026-11-22_mac_even_loss154.txt`): 91-116 s on
each opening move, 37 s on the losing move 85, which our engine gets right with 35 s+ more. The clock
allocation was `time / 30 + byoyomi` for every move (`usi.rs`), with no use of how the search was going (R5).

- New (`src/search/move_time.rs`): with main time left, a soft target `time / 25 + inc + byoyomi` and a hard
  ceiling `time / 8 + inc + byoyomi`. Before each iteration the target is stretched by best-move changes
  (halved per iteration, from depth 5), by a score falling against this side's previous move (±400 cp
  scale, 0.8-1.6), and shrunk 0.8x once the best move has held 8 iterations; total 0.5-3x. A new iteration
  starts while under half the stretched target. Byoyomi only: unchanged. `SHOGI_TM_OLD=1`: the old
  allocation. Ponderhit keeps the old allocation. 1181 unit tests pass (Mac).
- On the game's move 85 it did not help: the best move changes only at depth 19 after ~35 s.
- **Test (PC `s46tmgate`): new vs the same binary with `SHOGI_TM_OLD=1`, 4 threads each, 60 s + 1 s byoyomi,
  256 MB, 4 streams, seeds 60000-60003. PASS, LLR +3.20 after 232 games: 128-9-95, 57.1%, +50 ± 23**
  (simple SE; early stopping biases it up). Mean think 1139 vs 1147 ms a move (82 vs 83 s a game): the same
  time, spent differently. No time losses. 4 streams share 8 cores with SMT, equally for both.
- Not measured: the user's 30-60 min + 10 s (main time is 360x byoyomi there, 60x here), 16 threads.

### Step 2, the rest of the bundle (worktree `s46bundle`, d4ac4c3 + `matches/s46/bundle/b2.patch`, 2026-11-23)

The time manager shipped first (d4ac4c3, 2026-11-23, on the user's OK; `engines/usi-engine-stable`, previous
stable kept as `engines/usi-engine-7b8b1a0`).

Every repair is behind its own switch, off by default; all off = d4ac4c3 (0 of 40 fingerprints differ, Mac).
D3 and D5 from round 1 now default off as well (`SHOGI_SEE_PROMO=1`, `SHOGI_Q_KING_FIRST=1`). Each has a unit
test that fails on the old behaviour.

| | Switch | Change | Fingerprints (of 40) |
|---|---|---|---|
| D4 | `SHOGI_PERPETUAL_BAND=1` | perpetual check scores ±(30,000 − ply), the hand rule's band: kept out of the table and correction history, no early stop at > 50,000, below a real mate | 0 (no perpetual in the set) |
| D6 | `SHOGI_TT_BUCKETS=1` | 4-entry buckets; the same position keeps the one-way rule, otherwise the least valuable entry is always replaced (empty, then earlier search, then shallowest) | 40 |
| R3 | `SHOGI_ADOPT_FAIL_HIGH=1` | a completed root fail-high is played if the clock cuts off its re-search | 0 (needs a clock) |
| D7c | `SHOGI_SKIP_TWINS=1` | interior and quiescence nodes skip non-promoting pawn, bishop and rook moves that could promote (root, legality and USI keep them) | 40 |
| D1 | `SHOGI_CORR_GAIN_CAP=1` | correction update's depth term stops at gain 1 (depth 4 at the defaults) | 40 |
| D7a | `SHOGI_ASP_WINDOW=<cp>` | aspiration half-width (was a fixed ±200) | 40 at 60 |
| D7b | `SHOGI_ROOT_CHECK_EXT=1` | a non-losing checking root move gets the +1 it gets below the root | 0 here; changes a position with safe checks |

Not changed: D8 (a quiescence result found with checks used at a node searched without them is the more
thorough search, not an error).

**D6 offline check (PC, 1 thread, depth 16, 10 positions of g1108, `matches/s46/bundle/tools/depthcheck.py`):**
nodes to depth 16 were 20.1M (1 MB), 29.1M (16 MB), 28.8M (256 MB) direct-mapped; 24.5M, 27.3M, 27.8M with buckets.
Positions swing ±50% each way. A starved table reaches a depth with fewer nodes (fewer singular extensions, which
need a table entry), so nodes-to-depth cannot rank table designs. D6 has to be judged by games at a saturating hash.

**Screens (PC, build b2 md5 11c1cc09, one switch on vs all off, same binary, a1, colour-swapped pairs, simple SE;
`matches/s46/bundle/tools/screen.py`, PC `s46b2scr`, `s46b2clk`):**

| Arm | Conditions | Seeds | Result | Elo |
|---|---|---|---|---|
| D7c twins | 1 thread, 400k nodes, 256 MB | 61000-11 | 123-14-103 of 240, 54.2% | +29 ± 20 |
| D1 correction cap | same | 61100-11 | 118-18-104 of 240, 52.9% | +20 ± 21 |
| D7a aspiration ±60 | same | 61200-11 | 112-10-118 of 240, 48.8% | −9 ± 23 |
| D7b root check extension | same | 61300-11 | 117-14-109 of 240, 51.7% | +12 ± 19 |
| D6 buckets + R3 fail-high | 1 thread, 1 s byoyomi, **2 MB** (≈4.5 stores per entry a move, as the user's 16-thread 30-60 s moves at 256 MB) | 61400-09 | 110-9-81 of 200, 57.2% | **+51 ± 23** |

Clock arm: no time losses, 725 vs 732 ms a move. D6 and R3 were not separated. The fixed-node arms cannot see D6 or
R3; the gate (1 s, 4 threads, 256 MB) barely fills the table, so it sees R3 but little of D6.

The bundle for the gate: D7c, D1, D7b, D6, R3, D4. Out: D7a (±200 stays), D5 (−40 on the rung), D3 (neutral, not
screened). Screens have overstated before (S42: +63 → +11), so the sum is not a forecast.

**Gate candidate b3** (`matches/s46/bundle/b3.patch` on d4ac4c3; Mac md5 6ee2f79b): D4, D6, R3, D7c, D1, D7b on by
default, each `=0` to switch off. All six off = d4ac4c3 (0 of 40 fingerprints differ). 1187 unit tests pass (Mac). One
test changed: `quiescence_finds_repetition_with_the_path` built its repetition from an unpromoted rook's move in the
zone, a skipped twin; it uses a dragon now and passes with twins skipped or kept.

**Gate PASS 2026-11-23 15:25** (PC `s46b3gate`, `matches/s46/bundle/gate/final/`): b3 vs the same binary with its six
switches off (0 of 20 PC fingerprints differ from the d4ac4c3 build), 4 threads each, 1 s byoyomi, 256 MB, seeds 62000 /
62001, pooled SPRT [0, 40]: **261-22-211 of 494, 55.1%, +35 ± 14** (pair SE), LLR +2.98. Streams 131-14-99 and
130-8-112. No time losses; 727 vs 732 ms a move. The run dipped to LLR −1.54 at 132 games (48.5%) before recovering.
This gate fills the table little, so it measures mostly the other five; D6's own evidence is the 2 MB clock screen.

**16-thread check, started 2026-11-23 19:29** (PC `s46b3t16`, `matches/s46/bundle/t16/`; the §6.3 check before shipping):
the same pair, 16 threads each, one game at a time, 2 s byoyomi, **32 MB**, MoveOverhead 100, `--time-loss-ms 200`,
100 games, seed 63000. PC speed on a middlegame position: 0.40M nps at 1 thread, 3.95M at 16 (9.9x). At about 1.46 s
a move that is ~20x the nodes of the 2 MB 1-thread 1 s screen, so 32 MB fills about 5x a move, as 256 MB does in the
user's 16-thread 30-60 s moves. Read as a guard: 100 games have SE ~35-40 Elo, so it can catch a serious regression,
not size the gain. The Mac controller runs the gate's SPRT [0, 40] on it and stops at a decision (40 games minimum).

**16-thread check done 2026-11-24 02:08** (`matches/s46/bundle/t16/final/`): **49-12-39 of 100, 55.0%, +35 ± 30** (pair
SE; 95% −22 to +94), LLR +0.72, undecided at the cap as planned. No time losses; 1576 vs 1591 ms a move. As black 28-7-15,
as white 21-5-24. The same +35 as the 4-thread gate, now at 16 threads with a table filled ~5x a move: no sign of a
16-thread or saturation regression. It cannot size D6 on its own.

### Step 3 re-measure on the shipped bundle (Mac, started 2026-11-24 05:15, `matches/s46/remeasure2/`)

17a2e37 (bundle + time manager) with the 22 November rung settings and seeds 58000-58011, so each row pairs with the
7b8b1a0 row above: ours 800k vs YaneuraOu halfkpvm 100k on halfkpvm (yo-diagnostic build) and on a1, then 1.6M (x16) on
halfkpvm. The §5 rule reads the x8 halfkpvm row: >= 45% keep lean and tune it; < 45% step 4.

| Row (240 games each) | 7b8b1a0 (22 Nov) | 17a2e37 (24 Nov) | Change |
|---|---|---|---|
| ours halfkpvm 800k vs 100k | 46.0% (−28 ± 22) | 48.1%, 107-17-116 (−13 ± 22) | +15 ± 31 |
| ours a1 800k vs 100k | 40.8% (−64 ± 23) | 46.7%, 110-4-126 (−23 ± 22) | +41 ± 32 |
| ours halfkpvm 1.6M vs 100k (x16) | 50.2% at 16x in §1.1 (pre-delta-fix, other openings) | 70.0%, 166-4-70 (+147 ± 24) | |

Done 08:02. x8 halfkpvm at 48.1% clears the §5 line (45%) by about one SE: **keep lean and tune it** (step 4 not
started). x8 to x16 is +160 Elo, as one doubling has been worth before (§1.1). Interpolating, parity on the same
network is about 8.5x (2^(13/160) x 8): ~3.1 doublings of search left, from ~3.2 on 22 November and ~4 before the
delta fix. a1 now trails halfkpvm in our search by ~10 Elo at 800k (was ~36).

### Council (2026-11-24, direction after step 3)

Members: Claude Opus 5.5 (high), GPT-6.2 Sol (high), Grok 4.7 (high, Cursor; replaces Gemini at the user's request,
Gemini usage out). Brief `matches/s46/council_direction_1124.md`. One round, no scores (direction mode).

- **Agreed:** a tuning pass on lean cannot close ~3.1 doublings (+50 on the rung is ~0.3). The search interior is the
  main lever (Opus and Grok rank it first, Sol second). Network data/training is the parallel PC lever (all three, #2
  or #3); a wider net is doubtful (speed cost; tanuki 1024 wide +10 at 65% nps). The +149 self-play vs +15 rung is a
  denominator effect, not a contradiction: the delta fix is inside the 46.0% baseline, the time manager and R3 need a
  clock, D6 needs a full table; what fixed nodes can see (twins, D1, D7b) screened ~+61 and measured +15/+41.
  The 1-thread 100k rung alone is not the yardstick for 16-thread 10-100M-node games.
- **Differed:** Sol wants one bounded lean tune first (improving + quiescence evasion pruning replicated, then 8-12
  pruning/LMR knobs at 200k-400k), the core as the larger bet; Opus and Grok fold tuning into the end of an interior
  rebuild (lean's margins x0.6/x1.6 moved < 5 points, §1.3; S40 SPSA moved little). Lead: the interior rebuild, with
  Sol's replication run first as a cheap input to it.
- **Checked by the lead in the code (17a2e37):** check and singular extensions +1, capped only by
  `check_extensions_on_path < current_depth` (`lean.rs:796-801`); quiet checks never reduced (`late && quiet &&
  !mv.gives_check`, `lean.rs:859`); all shallow pruning gated by remaining `depth` (`lean.rs:747-790`); singular 559k
  vs 154k nodes to depth 12 (PLAN_S45 §13.2); k ≈ 29x in S41 (PLAN_S41 §7). Not checked: the field URLs and the
  YaneuraOu per-step figures Grok cites from RESEARCH_S40 §2c.
- **Partly did not hold:** Sol's "root and table first" — buckets and fail-high adoption already shipped; only
  persistent root records and table eval/was-PV remain, and Opus and Grok fold those into the interior stage.
- **Open, decides the size of the bet (Opus):** before the delta fix the parity ratio grew with depth (8x at 10k,
  16x at 100k, §1.1). If it still does, the user's games face ~5 doublings, not 3.1. Measure x8 at 10k, 30k, 300k.
