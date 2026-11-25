# PLAN S47: make the search scale with depth (2026-11-24)

Follows PLAN_S46 (repairs shipped as 17a2e37; council 2026-11-24, direction). Worktree `s47core`, branch
`s47-core`, from 17a2e37.

## 1. Stage 0: what the gap looks like now (Mac, `matches/s46/remeasure2/`, `matches/s47/under/`)

Ours (17a2e37, halfkpvm via the yo-diagnostic build) at 8x YaneuraOu V9 + halfkpvm's nodes, 1 thread, 240 games a row,
seeds 58000-58011:

| YaneuraOu nodes | Our score | Elo | Parity at about |
|---|---|---|---|
| 10k | 85.2% (204-1-35) | +304 ± 32 | 2x |
| 30k | 69.6% (164-6-70) | +144 ± 24 | 4x |
| 100k | 48.1% (107-17-116) | −13 ± 22 | 8.5x |

Each ~3.3x of nodes costs us ~155 Elo: the parity ratio doubles about every 3x. At the user's 10-100M nodes a move the
gap is far larger than the 100k row says (an extrapolation, not measured). The gap is in how the tree grows with depth.

**Where the nodes go** (search-diag, depth 12, a1, 40 positions of the S45 set, 20.4M nodes to depth 12):
- Share of all nodes with, above them on the path: a singular extension 91%, a check extension 66%, a late quiet check
  searched unreduced 46%, a late quiet move that pruning by reduced depth would drop 29%. This share saturates (one
  extension near the root tags its subtree), so it shows reach, not cost.
- Cost, nodes to depth 12 with the rule off: singular extension off 0.28x; check extension off 0.83x median-like
  (pooled; 8 positions that resolve instantly with it blow up without it).

**Replication (Sol, PLAN_S46 §1.3):** improving 7 + quiescence evasion pruning at 40k vs 10k, halfkpvm, 480 games
paired by opening, seeds 59000-59011: +2.8 ± 2.9 points (z 1.0), base 62.4%. The +55 did not replicate; not adopted.

## 2. Field evidence for stage 1 (public notes only; no GPL source read)

- YaneuraOu's search follows Stockfish's ("探索部のお手本にしているStockfish"); its 2025 changelog catches up to Stockfish
  of 2025-08-09 and 2025-11-13. The V9 overhaul was R50 at 1 s, R70 at 2 s, R90 at 4-8 s: the gain grows with time.
- Quiet-move futility in YaneuraOu is written in the reduced depth `lmrDepth` (changelog 2025-08-09). A Stockfish test
  note (2024-07-15): bigger lmrDepth thresholds were better at longer time controls.
- Stockfish removed the check extension (2024-02-09, passed as a simplification). It keeps singular extension with
  double/triple extension limits.
- Not found in public notes: LMR of checking moves in either engine. That arm rests on our own measurement.

## 3. Stage 1 (`SHOGI_S47` bit set in `lean.rs`, default 0 = 17a2e37)

| Bit | Change | Nodes to depth 8 (34 positions) |
|---|---|---|
| 1 | history, futility and SEE pruning of quiet moves by the move's depth after LMR | −38% |
| 2 | no check extension (root check extension D7b unchanged) | −15% |
| 4 | late quiet checks get LMR, one ply less than other quiets | −10% |
| 8 | negative singular extension: failed test with table score >= beta, or at a cut node | −2% |

Off: 34 of 34 fingerprints identical to 17a2e37; 1187 unit tests pass. Fewer nodes to a depth is not strength (S41).

**Judged on the rung** at 30k and 100k (x8, halfkpvm), paired with the stage 0 rows. Continue to stage 2 only if the
combination is clearly better at 100k (>= +40 Elo, about 2 SE) and the 30k-to-100k drop shrinks. Otherwise stop and
re-plan. Then self-play at the gate before any ship. Bits that hurt alone are dropped from the combination.

## 4. Stage 1 result at 30k (2026-11-24 09:10-10:17, `matches/s47/rung/`): no arm helps

x8 vs YaneuraOu halfkpvm 30k, 240 games, paired by opening with the 17a2e37 row (69.6%):

| Arm | Score | Paired difference |
|---|---|---|
| 1 prune by reduced depth | 61.7% | −7.9 ± 4.2 points (z −1.9) |
| 2 no check extension | 67.9% | −1.7 ± 4.1 (z −0.4) |
| 4 LMR of late checks | 67.9% | −1.7 ± 3.8 (z −0.4) |
| 8 negative singular extension | 62.3% | −7.3 ± 4.1 (z −1.8) |
| all four | 64.0% | −5.6 ± 4.1 (z −1.4) |

Stage 1 fails its rule; the 100k row was not run. Bits 2 and 4 are neutral (they remove nodes without a loss); bits 1
and 8 hurt as transplanted. Bit 1 cut 38% of the nodes to depth 8 with margins tuned for remaining-depth gating, so it
prunes far harder than before: untuned, as YaneuraOu's V9 changes were weaker until tuned. Whether the field's rules
pay here after tuning is not shown, and tuning is the expensive path.

## 5. What 8x is worth to YaneuraOu itself (2026-11-24 10:40-11:02, `matches/s47/yoself/`)

YaneuraOu V9 + halfkpvm at 8x against itself at 1x, same seeds and settings as the rung:

| Nodes (1x side) | YaneuraOu 8x vs itself | Ours 8x vs YaneuraOu | Gap at equal nodes (difference) |
|---|---|---|---|
| 10k | 98.5%, +732 ± 94 | 85.2%, +304 ± 32 | ~430 |
| 30k | 99.2%, +830 ± 123 | 69.6%, +144 ± 24 | ~690 |
| 100k | 94.8%, +504 ± 50 | 48.1%, −13 ± 22 | ~520 |

- §1's "the gap grows with depth" was mostly wrong: 8x is worth less at depth to YaneuraOu too (its rows sit near the
  score ceiling, so their Elo is coarse). Our 8x fell ~317 Elo from 10k to 100k, YaneuraOu's ~230 ± 110.
- The gap is a large, roughly constant inefficiency at equal nodes, ~400-700 Elo (≈ 3 of YaneuraOu's doublings at
  100k, ~170 Elo each), at every depth measured. Stage 1's 30k verdict stands; the rebuild is judged at 30k and 100k.

## 6. Council (2026-11-24, plan check of the tuning pilot)

Members: Claude Opus 5.5 (high), GPT-6.2 Sol (high), Grok 4.7 (high, Cursor; for Gemini, usage out). Brief
`matches/s47/council_plan_1124.md`. Grok returned only a two-line note (same two defects); no full review.

- **Agreed: hold the PC night.** Bit 1 as written is defective, so stage 1 did not test the field's rule:
  - `prune_depth = (depth - 1 - quiet_r).max(0)` counts an unreduced move a ply shallower than the old rule, and can reach
    0 where the loop's clamp keeps the child at >= 1. At 0, SEE pruning becomes "any losing quiet" and futility drops to
    `fut_base` (checked: `lean.rs:789-793` vs the loop's `r.clamp(0, new_depth - 1)` and `late` at `lean.rs:909`).
  - `quiet_r` includes `-quiet_score / lmr_hist_div`; `quiet_score` sums up to three tables capped at 16,000
    (`lean_hist.rs:41`, `:205`), so history can push the pruning depth down by ~6 plies (checked).
  - The pruning path ignores the loop's second-PV-move exemption (checked, minor).
  - History pruning is off in the shipped engine (`SHOGI_LEAN_HIST` default 3; pruning needs bit 4, `lean_hist.rs:37`),
    so `hist_prune_depth` / `hist_prune_mul` are inert (checked).
  - Stage 1 was judged on halfkpvm with margins tuned on a1 (scale ~1.75x): the scale is mixed in.
- **Adopted:** fix bit 1 (pruning depth = the depth the loop actually searches the move to, unreduced = the old rule, no
  history term), re-judge on a1 at 30k paired with a new a1 base row; spend a PC night only if the fixed arm is at or
  above −1 SE and keeps a real node saving. Any SPSA then tunes the five live constants at 100 ms (fixed nodes would take
  ~65 PC hours for 20,000 pairs, Sol) with `--corr`, beside a control run of the shipped search with the same budget, and
  a held-out 100k a1 checkpoint at 5,000 pairs: stop if its one-sided upper bound is below +40.
- **Pass odds of the pilot as first written:** Opus 10%, Sol 5-15%. Both: most outcomes would have decided nothing.
- **Circles:** both say the "untuned" excuse was being reused; a pre-registered rule ends the bit 1 line if it fails.

## 7. Fixed bit 1 on a1 (2026-11-24 11:44-12:06, `matches/s47/rung/`)

Fix: pruning depth = 1 + the depth the loop searches the move to (same eligibility and clamp as its LMR, no history
term); unreduced moves are judged as before. Off: 34/34 fingerprints identical; 1187 tests pass. Nodes to depth 8:
−21% (the defective version −38%).

x8 vs YaneuraOu halfkpvm 30k, ours on a1, 240 games, seeds 58000-58011, paired by opening:
- base (17a2e37 search): 58.8% (140-2-98), +61 ± 23
- fixed bit 1: 55.0% (130-4-106), +35 ± 23; **paired −3.8 ± 4.5 points (z −0.83)**

Pre-registered rule (§6): at or above −1 SE with a real node saving → a tuning night may be proposed. It passes, narrowly.

## 8. Tuning pilot (PC, started 2026-11-24 12:24, `matches/s47/tune/`, PC `~/s47tune`, `~/s47spsa`)

Two SPSA runs at once on 327541d (WSL build), 8 workers each, 20,000 pairs, 100 ms byoyomi, overhead 20, 1 thread, a1,
correction on (`--corr`, weights 2,2,3,7 as shipped): **bit1** (`SHOGI_S47=1`) and **ctrl** (shipped search). Five live
constants from their shipped values (`params5.json`: fut_depth 6, fut_base 106, fut_mul 153, see_quiet_depth 6,
see_quiet_mul 86; S40 ranges and step sizes). ~65 pairs a minute each: ~5 h.
- Checkpoint at 5,000 pairs: bit1's theta at x8 vs YaneuraOu 100k, a1, paired with the 17a2e37 a1 row (46.7%). Stop if
  the one-sided 95% upper bound is below +40.
- Verdict after 20,000: both thetas at 30k and 100k on a1. Pass: tuned bit1 >= +40 over 17a2e37 at 100k and above
  tuned ctrl. Otherwise the bit 1 line ends.

**Checkpoint (2026-11-24 13:41-14:20): stop.** bit1's theta at 5,150 pairs (fut_depth 6, fut_base 105, fut_mul 155,
see_quiet_depth 7, see_quiet_mul 85: barely moved; ctrl's barely moved too) at x8 vs YaneuraOu 100k on a1: 37.1%
(86-6-148) against 17a2e37's 46.7%, **paired −9.6 ± 4.2 points (z −2.3), about −67 Elo; one-sided 95% upper bound
−19**, below the +40 line. Runs stopped at ~7,800 pairs each (14:21).

- Reduced-depth pruning hurts more at 100k than at 30k (−3.8 ± 4.5 untuned at 30k), the opposite of the field note
  that it pays more at longer time. SPSA at 100 ms found nothing to move.
- **The bit 1 line ends** (rule of §6). Bits 2 and 4 were neutral at 30k on halfkpvm; bit 8 hurt. Stage 1 as a whole
  gave nothing.

**Singular extension row (2026-11-24 14:25): stopped unrun.** The user pointed out that a feature's value in lean does
not carry to a rewrite structured differently, so the row could not decide anything about the new core. Agreed; the
same applies to stage 1's verdicts. Stage 1 is closed; the rewrite goes ahead (§9).

## 9. The rewrite (PLAN_S46 §4 design), proposed 2026-11-24

A new search, `src/search/core/`, selected by `SHOGI_CORE=1` (default off: the shipped lean search stays the control),
entered from the root loop of `search_at_depth` where lean is now (`search_engine.rs:6826`). Reused unchanged: board,
make/unmake, move generation and legality, NNUE, the bucketed table, mate solver, repetition and hand-rule definitions,
USI, time manager. Ideas from public notes only; no GPL source, no other engine's constants.

| Milestone | Content | Check |
|---|---|---|
| M1 (~3-4 days) | per-ply stack; table entry with static eval and was-PV; staged move picker (table move, good captures, quiets by history, bad captures; evasions own order); one stats module (main, continuation, capture histories, correction); node: PVS, eval refined by table bound, reductions as one sum in fractional plies, pruning by reduced depth, singular/negative/double extension, no check extension; own quiescence, table-backed; all constants in one params table | shogi rule tests (perpetual check, superior/inferior repetition, pawn-drop mate, no-move loss, mate distances, `searchmoves`, stop latency); legal play over 200 games; 4 threads |
| M2 (~1 day) | starting constants from our own statistics, scaled to a1 | rung 30k a1 sanity row: expected weaker than lean untuned |
| M3 (2-3 PC nights) | SPSA in phases of 8-20 constants, a1, fixed-time 100 ms then a slower confirm | held-out rung row per phase |
| M4 | verdict: x8 vs YaneuraOu 100k, a1, 480 games paired with lean | the rule below |

**Verdict rule (PLAN_S46 §5 kill rule, kept):** after M3, at 100k x8 over 480 games on a1: >= lean + 40 Elo: continue to
the gate; within ±40: one more tuning phase; below lean − 40: stop the rewrite. No design choice is judged in lean.

## 10. Council (2026-11-24, direction: three routes to parity)

Members: GPT-6.2 Sol (high), Grok 4.7 (high, Cursor; for Gemini). Claude Fable 5.2 (asked for in place of Opus) failed
twice on an API rate limit: no review. Brief `matches/s47/council_parity_1124.md`. No GPL source was read.

| Route | Sol: parity at equal hardware, 3 / 6 months | Grok | Expected |
|---|---|---|---|
| 1 independent rewrite + speed + network | 1-5% / 5-15% | 10-15% / 25-35% | +160-400 (Sol), +200-450 in ~4 months (Grok) |
| 2 GPLv3: C++ fork of YaneuraOu + our network | 45-65% / 65-80% | ~40% / 65% vs halfkpvm; 20% / 45% vs Suisho5 | +550-800 (Sol); residual −80 to −250 (Grok) |
| 2b GPLv3: Rust port | 10-20% / 25-45% | (4-8 weeks to a depth-agreeing port, then hot loops) | |
| 3 target the user's 16-vs-4 settings | 1-3% / 3-10% (20-40% for the narrower target at 6 months) | a milestone, not a goal | |

- **Agreed:** ranking 2, 1, 3. Route 3 is a lower bar (a public opponent is not held to 4 threads); use it as a
  milestone. A port is a derivative work: private use is free, a public download requires the whole engine's
  corresponding source under GPLv3. Dependencies are MIT/Apache (compatible); networks trained on nodchip's MIT data
  stay ours; Suisho5 weights need their own clearance. `apery_rust` is GPLv3 (lead checked the local LICENSE);
  `rshogi-core` GPL-3.0-or-later (both members, public page); Sekirei MIT/Apache with no evidence near V9 (Sol).
- **Both say the lead's +300-550 for route 1 is too high;** its upside, not its expectation.
- **Grok:** "~3 doublings at every depth" (§5) is not supported: the parity ratio grew 2x, 4x, 8.5x (§1) and the 10k/30k
  YaneuraOu self-play rows are at the score ceiling. The gap past 100k is unmeasured. Lead agrees; §5's reading is
  withdrawn to "unknown beyond 100k, possibly growing".
- **Early checks:** route 1: the untuned core's 30k a1 row and nodes to depth 12; skip tuning if > ~150 Elo behind lean
  with ~5x nodes per depth (Grok). Route 2: move agreement with the YaneuraOu binary on 200 positions at depth 8/12
  (< ~90% or node ratio outside 0.5-2x: stop, Grok); deterministic one-thread PV/score/node match with our evaluator
  (Sol).
