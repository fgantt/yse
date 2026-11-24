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
