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
> 3. **Stable is now `c0b2a03`** (`engines/usi-engine-stable`; `-stable-prev` = `11220d2`,
>    `-stable-prev-old` = the one before). `target/release` (the GUI's) untouched.
> 4. **Threat inputs in the network (`9d77f1c`): six-piece SPRT FAIL**, 33-3-44,
>    −48.1 ± 58.9 (below). The control fine-tune (same data, no threat inputs) is neutral,
>    118-10-118, 0.0 ± 32.3: the threat inputs (or their 17% speed cost) lost, not the data.
> 5. Session 32's READ THIS FIRST still holds otherwise: the open problem is the evaluation of
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

## A four-piece loss (stable `11220d2` era build in the GUI)

`matches/user_games/2026-10-08_4mai_loss57.txt`, lost in 57. YaneuraOu (3M nodes, 4 threads)
against our stable at 1 s, every position (`matches/user_4mai_20261008_analysis.jsonl`):
YaneuraOu about +2,500 through move 38 (the 2026-10-06 four-piece losses started sliding at
moves 24-32). Then `S*5a` (39, −1,240), attacking drops while our king walked
7i-8h-7g-7f-8e-9f; even at move 47, where ours read +1,300; `8e9f` (51) −3,600; mate by
`G*9e`. Ours stayed +800 to +1,300 from move 45 to 53 while YaneuraOu went from 0 to lost:
the king-danger blind spot again. Labelled into `eval_gap/positions_s34.jsonl` (114).

## Threat inputs in the network (rejected)

`src/evaluation/nnue_threats.rs` (`9d77f1c`): 90 inputs per perspective, from the board at
every evaluation, added to the first layer before activation. For both kings' 3x3 zones:
enemy attackers per square (1/2/3+), attacked and undefended (no defender but the king), a
thermometer of attacked zone squares. A network has them when its row count is
`nbuckets * KING_FEATURE_STRIDE + 90` (no format change; without them the search fingerprint
is byte-identical). Trainer `--threat-features` appends zero rows; the king factoriser skips
them; every 1,000th training position is asserted equal to the engine's evaluation (no
mismatch in 4M).

Two fine-tunes of `nnue/default.bin`, Session 30 recipe (`corpus_hc/mix_x1.jsonl`, lr 0.001,
2 epochs, seed 47), identical but for the flag (`weights/session34/`, not in git):

| network | val r_clamp | train err | eval gap, our king high (82) | mid (102) | low (752) | nps (search_bench d5) |
|---|---|---|---|---|---|---|
| stable `default.bin` | | | 0.180 (+0.147) | 0.183 | 0.021 | |
| control | 0.778 | 0.1352 | 0.162 (+0.108) | 0.180 | 0.024 | 91.0k |
| threat | 0.781 | 0.1333 | **0.142 (+0.077)** | 0.177 | 0.023 | 76.0k (−17%) |

**Six-piece SPRT, threat vs stable** (`c0b2a03` both, 1 s/move, `--random-plies 2`, seed
3402): **33-3-44, −48.1 ± 58.9, LLR −3.25: FAIL** after 80 games
(`matches/s34_threat_6mai_sprt.*`). As Black (holding the odds): threat 28-10, stable 34-5.
As White: threat won 5, stable 10. Worse in both roles; as in Session 32, the network nearer
YaneuraOu's king-danger numbers converts the odds less often.

**Control vs stable, same openings and seed:** 118-10-118 after 246 games, **0.0 ± 32.3**,
LLR −2.97: FAIL against [0, 40], i.e. neutral (`matches/s34_control_6mai_sprt.*`). As Black:
control 100-15 (8 draws), stable 103-18 (2 draws). The fine-tune data and recipe are harmless;
the loss belongs to the threat inputs. Part of it is speed (−17% nps is worth roughly 15-20
self-play Elo at 1 s/move), the rest is unresolved within ±59.

## A second four-piece loss (stable `c0b2a03`)

`matches/user_games/2026-10-09_4mai_loss65.txt`, lost in 65, partly while the control SPRT
shared the CPU. YaneuraOu (3M nodes) on every position
(`matches/user_4mai_20261009_analysis.jsonl`): +2,700 to +3,000 through move 26, the best
opening so far. The bishop's diagonal stayed shut (YaneuraOu's first choice was `7g7f` at 9 of
our first 12 moves; played at 34), but the net opening cost was about 60. Slow bleed 27-46
(+2,750 -> +1,630: `5i5h` −305, `P*6f` −355, `N*3f` −416). Collapse at 47-52, about −5,000 in
six moves (`P*5c` −677, `6f6e` −957, `N*4e` −1,016, `S*3c` −427, `3c2d` −1,067) while White
built on the 7-file against our king on 8h; YaneuraOu wanted quiet king moves (`8h8i`,
`8h7i`). Ours read +1,200 to +1,900 through the collapse. Labelled into
`eval_gap/positions_s34b.jsonl` (130).

## A third four-piece loss, and why search is not the fix

`matches/user_games/2026-10-09_4mai_loss54.txt` (stable `c0b2a03`), lost in 54; analysis
`matches/user_4mai_20261009b_analysis.jsonl`. YaneuraOu +2,600 through move 53, then −4,000 in
moves 79-91 (`5h4h` −644, `2f1f` −470 trapping the rook, `5f6e` −444, `7c7d` −903, `G*3g`
−1,032) while ours read +600 to +1,400, our king walking 6h-5h-4h into White's attack.

Fifteen of our worst moves from the three games, searched again (`go byoyomi`, YaneuraOu
3M nodes scoring the chosen move):

- **More time does not help.** 1 s, 5 s, 20 s: 11 of 15 keep the same move, and our score of
  the position moves by under 200 cp where YaneuraOu says −1,500 to −3,700.
- **No pruning rule hides it.** Six collapse positions at 5 s with `SHOGI_NO_NULLMOVE`,
  `SHOGI_NO_LMR`, `SHOGI_NO_PRUNING` each: the same move every time, scores within ~250.
- **YaneuraOu's line is not a mate, and it is found by depth.** After `8e9f` (game 1, move 50)
  YaneuraOu says +3,740 for White at depth 18 (3M nodes), no mate. Walking its PV with ours at
  1 s per position: +893, +813, +753, +872, negative only from the fourth ply, mate seen at
  ply 16. Session 31 showed YaneuraOu's own network is as optimistic as ours at depth 1 on
  such positions and needs ~10 plies; I first misread this as "YaneuraOu's evaluation sees
  it at the root". It does not.
- **Our search is very narrow in depth, very wide in breadth.** Same position: ours reaches
  depth 4 in 1 s (184k nodes) and depth 7 in 20 s (11.9M nodes); YaneuraOu reaches depth 4
  at 2,000 nodes and depth 18 at 3M. Effective branching ~10 against ~2.3. That, not a
  single pruning rule, is why 20x the time changes nothing. (Nominal depths are not
  comparable across engines -- YaneuraOu's reduces far more per ply -- so the ratio is the
  signal, not the exact factor.)

So neither a search-side king-danger rule nor a king-danger training set (Session 31 tried
that: bias fell, per-position error rose) addresses it. The gap is tree shape.

## Threat net against YaneuraOu (six-piece, 2,000 nodes)

Session 32's rung: start `2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1`, ours at
1 s, YaneuraOu `nodes 2000` (1 thread, no book), `--random-plies 2`, seed 2910, 80 games each,
both run side by side (`matches/ladder_s34/`). Only our games as Black count:

| engine | Black W-L-D | score |
|---|---|---|
| Session 30 stable | 22-13-5 | 61% |
| Session 32 stable | 29-7-4 | 78% |
| stable `c0b2a03` | 28-7-5 | 76% |
| threat net | 31-8-1 | 78% |

Same openings, paired: 23 both won, 5 only stable won, 5 only threat won; stable's draws went
3-1 to threat. **A tie within noise** (40 games resolve about ±13%). Two readings: the threat
net's −48 in self-play does not show against YaneuraOu, and the +74 self-play Elo of the speed-ups
does not show either (76% vs 78%). Self-play against stable is a poor proxy for this rung.

Four-piece rung, same settings (start `1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1`,
YaneuraOu 2,000 nodes), 120 games each (`matches/ladder_s34/yonmai2000_*`):

| engine | Black W-L-D | score |
|---|---|---|
| stable `c0b2a03` | 31-26-3 | 54.2% |
| threat net | 30-25-5 | 54.2% |

**An exact tie.** Paired by opening: 14 both won, 9 both lost, 15 only stable won, 14 only
threat won. The opening predicts little (random plies, timing), so pairing does not cut the
noise much; 60 counted games resolve about ±12%.

**Verdict on threat inputs: neutral against YaneuraOu on both rungs, −17% nps, −48 in
self-play. Not shipped.** This rung (54%) responds, unlike six-piece; it is the measurement to
use for king-danger work, at 60+ counted games per engine.

## Pruning gated on the static score (`SHOGI_PRUNE_PLAIN`, opt-in)

`PruningManager::should_apply_conditional_pruning` derives "king safety", "complexity" and
"tactical potential" from the static score alone: a side more than 300 behind is "king in
danger" and gets no futility, razoring or late-move pruning; |score| feeds complexity, so a
lopsided endgame gets none either. In a handicap game that is White at nearly every node.
`SHOGI_PRUNE_PLAIN=1` replaces the gate with `move_number >= 3` (default unchanged).

Game-1 position after `8e9f`, nodes to depth 7: 18.1M -> 6.1M. The 15 mistake positions: the
same move in 14 at 1 s; mean YaneuraOu score after our move −306 (stable) vs −259 (1 s),
−348 (5 s): no help there.

| test | stable | plain |
|---|---|---|
| six-piece SPRT vs stable (seed 3403) | | **PASS**, 32-2-22, +62.7 ± 48.9, LLR +3.02 after 56 games |
| four-piece vs YaneuraOu 2,000 nodes, Black games (seed 2910) | 31-26-3, 54.2% | 28-30-2, 48.3% |

Paired by opening (four-piece): both won 15, both lost 16, stable only 14, plain only 10.
**They disagree**: self-play +63, YaneuraOu −6 points (±12, not significant). Not shipped; the
SPRT stopped at 28 pairs, under the spirit of the minimum-pairs rule.

Second batch, seed 2911 (`*_b`): stable 36-24-0 (60.0%), plain 30-26-4 (53.3%). **Pooled, 120
counted games each: stable 57.1%, plain 50.8%** (−6.3, about one standard error). Against
YaneuraOu it leans negative; the self-play pass did not transfer. Rejected.

## Next

1. King-danger evaluation (threat inputs in the network), gated by the six-piece handicap
   SPRT.
2. More exact speed: an accumulator refresh cache per king square (Finny tables) for
   `refresh_pov`; quiescence still generates every board move to find promotions and checks
   (a direct promotion/check generator, as done for the threat count).
3. Re-measure parallel nps at 1 s/move (the engine we ship) against stable.
