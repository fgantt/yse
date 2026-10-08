# NNUE Implementation Session Log

## Session 34: Exact Speed-ups (single-threaded +51%)

**Date**: 2026-10-08
**Phase**: Phase 2w
**Objective**: Engine speed, by changes that leave the single-threaded fixed-depth search
byte-identical -- they cannot lose strength, so they stack and are gated once -- before going
back to king-danger evaluation.

> **READ THIS FIRST** (state at the end of this session).
>
> 1. **Five exact speed-up commits** (`1c59f8d` .. `c0b2a03`): `search_bench --depth 5` over
>    the default 30 positions with `nnue/default.bin`, fingerprint byte-identical after every
>    commit, **60.7k -> ~92k nps** (+51%, separate runs; paired A/Bs per step below).
> 2. **Gate: PASS.** SPRT against stable (`11220d2`), 1 s/move, [0, 40], seed 3401:
>    104-0-68 after 172 games (86 pairs), paired **+73.8 +/- 54.6 Elo**, LLR +3.05
>    (`matches/s34_speed_sprt.*`). Self-play; speed gains overstate against YaneuraOu.
> 3. Session 32's READ THIS FIRST still holds otherwise: the open problem is the evaluation of
>    king danger, judged only by games.

---

## Method

Profile `search_bench` single-threaded with `sample` (40 s, top-of-stack and caller
attribution), change one thing, test it against a reference implementation where the change
is not obviously order-preserving, then interleaved A/B pairs on 12 positions (separate runs
drift by 2-5%), and the full 30-position fingerprint before each commit. Every result below
left the fingerprint byte-identical.

Baseline profile (31.4k busy samples): evaluation 25%, `generate_legal_moves` 23% (most of it
in check, inside quiescence), move ordering 15%, `count_tactical_threats` 8%, full-board
Zobrist rehash 6%, malloc/free ~6%.

## What changed

| commit | change | paired A/B |
|---|---|---|
| `1c59f8d` | Board keeps the XOR of its pieces' Zobrist keys up to date (`board_key`); `hash_position` no longer rescans 81 squares, and it is called several times per node | 60.7k -> 67.7k (all three) |
| | In check, a non-king move that neither captures the single checker nor lands between it and the king is rejected before make/unmake (double check: every non-king move) | |
| | Per-move `start_timing`/`end_timing` keys were `format!`ted with debug logging off | |
| `cd45f3b` | Tactical-threat count capped at the threshold callers compare it with (0 or 3); memo records exact vs lower bound | +4.5% |
| | SEE on a stack attacker list with the old `Vec`'s order, dedup and shifting removal | +5.7% |
| `23769f9` | Drops generated from the empty-square bitboard with nifu/rank masks; quiescence drops only where the piece could attack the king (a drop never uncovers a check), legal generation in check only on blocking squares. The nifu test pushed into a `Vec` for every square | +13.7% |
| | NNUE snapshot stack reuses its slots (push copies, pop swaps) instead of two 1 KB allocations per make | +0-3% |
| `82189e2` | `count_legal_captures_and_promotions` walks each piece's targets and builds only captures and promotions | +4-7% |
| | `generate_all_piece_moves_into` iterates occupancy bits; ray directions are static slices (a `vec!` per lance/rook/bishop) | +3-8% |
| | NEON layer 2 on two accumulator sets (even/odd rows) | ~+1% |
| `c0b2a03` | Capture generation appends in place (was a `Vec` per piece and an 81-square probe); quiescence dedup is a stable sort of the moves (was an index sort plus a copy) | +4-6% |

New tests guard each non-trivial change against the old code: incremental board key vs
rescan (make/unmake, drops, promotions, clone, FEN); evasion filter vs full test on
check-heavy games (>1,000 in-check positions, double checks included); `attackers_of` vs
`is_square_attacked_by`; masked drop generator vs the old one; legal generation vs the full
filter; capped counts vs exact; stack SEE vs the allocating version; i32 layer-2 kernel vs
i64 scalar.

## Tried and dropped

- **Branch-free layer 2** (collect nonzero activation indices, then a branchless loop): 1%
  slower. The zero/nonzero pattern is predictable enough.
- **NNUE evaluation cache** (32K direct-mapped, keyed by board key, hands, side to move and
  network id): no clear gain (-0.7%, +3.2%). Quiescence's own table already caches
  stand-pat; repeated evaluations are rare.

## Where the time goes now (v10 profile, 32.2k busy samples)

Evaluation 34% (forward pass 21%, `refresh_pov` on king moves ~5%), quiescence move
generation 20%, move ordering 20% (SEE ~5%), `generate_legal_moves` 12%.

## Method notes

- **`cargo test` has pre-existing failures** (8 lib tests: magic promoted pieces, prefetch,
  bezier endpoints, castles x2, config x2, major piece activity) -- identical at `537be03`.
  `test_simd_performance_against_baseline` and the popcount comparison are timing tests that
  fail under load and pass alone. The `nnue_basic_usage` example does not compile.
- **A checkout without `resources/magic_tables/magic_table.bin`** (gitignored) has no shared
  magic table in tests: SEE then sees no sliders and `see_scores_an_even_trade_as_even`
  fails. Environment, not code.
- **Promoted pieces fail `MoveOrdering::validate_move`**, so `score_move` errors (and builds a
  formatted error string) for every move by a tokin, promoted silver, dragon or horse; callers
  `unwrap_or(0)`. Fixing it changes ordering: a separate, gated change.

## Next

1. King-danger evaluation (threat inputs in the network), gated by the six-piece handicap
   SPRT.
2. More exact speed: an accumulator refresh cache per king square (Finny tables) for
   `refresh_pov`; quiescence still generates every board move to find promotions and checks
   (a direct promotion/check generator, as done for the threat count).
3. Re-measure parallel nps at 1 s/move (the engine we ship) against stable.
