# NNUE Implementation Session Log

## Session 31: Four Bugs That Did Not Matter, and One Blind Spot That Does

**Date**: 2026-09-27 to 2026-09-28
**Phase**: Phase 2w
**Objective**: Session 30's *Next* list: a 1 s/move handicap ladder, the bugs Session 30 left
alone because they change play, and the evaluation's known biases. Every change gated on a
head-to-head against `engines/usi-engine-stable`.

> **READ THIS FIRST.**
>
> 1. **Nothing shipped.** Stable is the Session 30 engine (`98052b4`). Four candidates were
>    gated (five matches, 600 games); none beat it. Two more (king danger) were running at
>    write-up.
> 2. **The four "left as found" bugs are real and fixing them does not help.** The game-phase
>    quirk (two fixes), the quiescence dedup, and pseudo-legal quiescence moves: neutral, neutral,
>    worse, and worse over 240 games (−42 ± 42). See *The gates*.
> 3. **Our evaluation is blind to attacks on its own king.** A user's six-piece loss turned on a
>    move the engine still chooses at 60 s / depth 8 (+294 cp; YaneuraOu −586). Across all six
>    user games, our win-probability error is ~9x larger when our king is under pressure, always
>    optimistic, and two more plies barely reduce it. This is the finding of the session.
> 4. **`scripts/eval_gap.py` measures it in 6 seconds** (`66f44dd`), from 830 positions of the
>    user's games labelled by YaneuraOu. Use it before any evaluation change is gated.
> 5. **It is not mate threats, and not attacked squares.** Only 1 of the 61 close, pressured
>    positions has a forced (all-checks) mate for the opponent. The worst errors have few
>    attackers near our king, 4-5 defenders, and 1-4 enemy pieces in hand: slow drop attacks on a
>    solid-looking castle. A squared attack-unit term removes the average bias without hurting
>    quiet positions, but does not track which positions are wrong (gates running at write-up).
> 6. **The handicap ladder rung is two-piece odds** at 1 s/move: 66% (26-1-13). Four-piece
>    (78%) and six-piece (79%, Session 30) are too easy to respond.

---

## The ladder at 1 s/move

`usi-stable` at `byoyomi 1000` against YaneuraOu at 50 nodes, 80 games per rung, our games as
Black (holding the odds) counted, `--random-plies 2`, seed 2910 (`matches/ladder_s31/`).

| rung | counted games | score |
|---|---|---|
| four-piece (`1nsgkgsn1/…`) | 30-2-8 | 78% |
| **two-piece** (`lnsgkgsnl/9/…`) | 26-1-13 | **66%** |

The four-piece run shared the machine with two gate matches; the two-piece run shared it with
one. Our engine's nodes per move depend on load, so compare later ladder runs under similar
load, or idle.

## The gates

All vs `usi-stable`, 120 games, `byoyomi 1000`, `--random-plies 8`, colour-swapped pairs,
seed 3001 unless noted. Patches are in the session scratchpad; none is committed.

| candidate | result | verdict |
|---|---|---|
| phase A: move orderer's phase from the real game ply, set once per search | 61-1-58, +8.7 ± 59.2 | neutral |
| phase B: no phase switching (default weights) | 52-0-68, −46.6 ± 63.4 | rejected |
| qdedup: quiescence keeps promotion twins, drops dominated P/B/R non-promotions, no duplicates | 58-3-59, −2.9 ± 48.5 | neutral |
| qlegal: quiescence promotions and checks filtered for legality | 50-1-69, −55.5 ± 60.2 | — |
| qlegal, seed 3002 | 53-4-63, −29.0 ± 59.4 | rejected (240 games: −42 ± 42) |

### Game phase

`initialize_advanced_move_orderer` passed the node count as `move_count`, so after a search's
60th node every position used the Endgame ordering weights. The phase only rescales quiet
moves within their tier (history against the castle / storm / positional bonuses); PV,
captures, promotions, killers and counter-moves are fixed tiers. Phase A (the intended
behaviour, via a process-wide game ply set in `handle_position`) is neutral. Phase B (default
weights: history 2500 against bonuses ~100) is worse, so the Endgame weights the engine
reached by accident are better than the defaults. Left as is.

### Quiescence dedup

The dedup removed only *adjacent* duplicates by (from, to, piece), ignoring the promotion flag.
A capture that checks is listed as a capture and as a check, and the two sort apart, so both
survived and were searched twice (test: `8e7c+` twice); a silver, knight or lance capture lost
one of its promotion twins. The fix (`quiescence_dedup_respects_promotion` fails without it)
is neutral in play.

### Quiescence legality

When not in check, quiescence took promotions and checks from the pseudo-legal list and plays
what it is given: a pinned piece could promote or give a discovered check and leave its own
king capturable. The fix costs nothing (depth 5 over 42 positions: 118k vs 117k nps, 3% fewer
nodes) and is worse over 240 games. A guess, untested: the bug cut both ways, and the
opponent's illegal checks in our quiescence made the search more pessimistic about our king,
offsetting the blind spot below. Retry once the evaluation sees king danger.

**Correct is not the same as better.** All four are plainly wrong in the code, and none is
worth Elo at 1 s/move. The user's games are lost elsewhere.

## A six-piece loss

The user played the stable engine against YaneuraOu with six-piece odds and lost in 71 moves
(`matches/user_games/2026-09-27_6mai_loss.txt`). YaneuraOu judging each of our moves at 3M nodes
(numbering is the game's; White moves first):

| move | played | YaneuraOu before → after | loss | YaneuraOu |
|---|---|---|---|---|
| 25 | 4e4d | 3138 → 2805 | 333 | 8g8f |
| 34 | 6h7i (king towards White's tokin) | 2578 → 1970 | 608 | 8i7g |
| 55 | 2d2f | 1243 → 827 | 416 | 3f4e |
| 57 | 2e2f | 864 → 408 | 456 | 2e1e |
| **63** | **6c5c** | **232 → −605** | **837** | 7h7g |

The same shape as Session 30's 77-move loss: +3,200 bled away over 60 moves, then one move
decided it. Move 63 trades the gold attacking White's king for a silver and hands White a gold
to drop beside our king (6h, with White's horse on 8i and tokins on 8h/7h). Our engine chooses
it at 10 s (+418) and at 60 s, depth 8, 10.8M nodes (+294). Depth does not fix it.

Our 1 s score against YaneuraOu's along the game: ~0.5x YaneuraOu's centipawns while winning
(the known compression), then flat at +430..+1,200 while YaneuraOu fell from +1,500 to +185 as
White's attack built (moves 46-63), and +420..+735 at moves 64-66 where YaneuraOu says −867 to
−2,365.

## The eval-gap set

`scripts/eval_gap_build.py` labels every position of `matches/user_games/*.txt` (both sides to
move) with YaneuraOu at 3M nodes, one thread, and tags each king from the board alone:
*pressure* = enemy pieces within two squares + enemy non-pawn pieces in hand (low 0-1, mid 2-3,
high >= 4). 830 positions from six games (one six-piece loss, five eight-piece games from
Session 30; the 595-move draw is cut to its first 120 moves).

`scripts/eval_gap.py ENGINE [--option NNUEFile=…] --depth 1` searches each position at a
fixed depth, single-threaded, from a fresh game, and compares in win probability,
`p = 1/(1+exp(-cp/410))` -- the trainer's `--sigmoid-eval-scale` for both sides, which is what
`--use-sigmoid-loss` trains to match. `--compare a.csv b.csv` is paired. Depth 1 takes 6 s.

Stable (`eval_gap/results/stable_d1.csv`):

| subset | n | mean abs dp, depth 1 | depth 3 | mean dp |
|---|---|---|---|---|
| our king, low pressure | 676 | 0.017 | 0.016 | 0.00 |
| our king, mid | 82 | 0.152 | 0.127 | +0.07 |
| our king, high | 72 | 0.160 | 0.136 | +0.12 |
| close (abs YaneuraOu < 1500), our king mid+high | 61 | 0.166 | | +0.09 |

In handicap games a close position is nearly always one where our king is under attack (61 of
66). **The subset to fix is the last row**; the pressure rows alone partly measure centipawn
scale in won positions. Session 30's linear-win-term networks cut the pressure rows from 0.160
to ~0.115 and leave the close subset at 0.19-0.20 -- they rescale won positions and lost
35-158 Elo. The handicap-data fine-tune (`x1`) moves the close subset 0.194 -> 0.189.

**Limits.** 61 close positions from three games, 44 from one. Neighbouring plies are
correlated, so the paired SE `--compare` prints is optimistic. Triage only; gates decide.

## What the blind spot is not

- **Not forced mates.** With White to move, `mate_solve` (1M nodes) finds a mate in 1 of the 61
  close, pressured positions. The errors are elsewhere.
- **Not attacked squares.** Over the 61, the per-position error correlates weakly with every
  board tag: enemy pieces near our king −0.23, enemy hand +0.17, enemy promoted pieces near
  +0.18, our adjacent defenders +0.24. The largest errors (dp +0.35 to +0.81) have 0-2 enemy
  pieces near our king, 3-5 defenders, and 1-4 enemy pieces in hand: drop attacks against a
  castle that looks solid, several quiet moves deep.

### A king-danger term (gating at write-up)

`src/evaluation/king_danger.rs` (uncommitted): attack units on a king = 2 per adjacent square
the enemy attacks + the enemy's hand (2 for G/S/R/B, 1 for N/L); the term is
`scale * units^2 / 64` per king, added to the NNUE score; USI option `KingDanger`, default 0.
At 0 the eval-gap output is identical to stable. Scale chosen on the six-piece game, checked on
the two eight-piece losses:

| scale | fit: 6-piece close+pressure | test: 8-piece close+pressure | low pressure (all) |
|---|---|---|---|
| 0 | 0.114 (+0.045) | 0.301 (+0.211) | 0.017 |
| 64 | 0.117 (+0.035) | 0.293 (+0.202) | 0.017 |
| 128 | 0.121 (+0.025) | 0.286 (+0.191) | 0.017 |
| 256 | 0.131 (−0.000) | 0.269 (+0.171) | 0.017 |
| 512 | 0.187 (−0.066) | 0.244 (+0.139) | 0.016 |

It removes bias and leaves quiet positions alone, but on the game it was fitted to, the error
*rises* as the bias falls: it does not know which positions are dangerous. Gates at 128 and
256 against stable were running at write-up: `matches/s31_kd128_h2h.log`,
`matches/s31_kd256_h2h.log`.

## Method notes

- **A 120-game gate resolves about ±60 Elo.** Small correctness fixes land inside it. When a
  result is borderline, a second match on fresh openings (different `--seed`) is cheaper than
  arguing about it (qlegal: −55 then −29).
- **`eval_gap.py` depth 1 is deterministic and reproducible** (`KingDanger 0` gives a
  byte-identical CSV to stable). Depth 3 adds little for evaluation questions.
- **Check an explanation against the positions before building on it.** "Mate threats"
  was the natural reading of the six-piece loss; 1 of 61 positions supports it.

## Left as found

- The four gated fixes (patches: `phaseA.patch`, `qdedup.patch`, `qlegal.patch` in the session
  scratchpad). qlegal is worth retrying after the evaluation changes.
- `determine_game_phase`'s node-count input and `record_timing` (no callers) are still there.
- The engine prints `DEBUG: Bestmove recommendation …` on stderr on every move.

## Next

1. The king-danger gates. If neither wins, the handcrafted route is probably done: the term's
   shape cannot see which castles are in danger.
2. Train for it instead. The network sees piece squares relative to its own king's bucket but
   hand pieces without the king; drop attacks need both. Candidates: king-conditioned hand
   features, or more positions like the close subset (the Session 30 handicap corpus was mostly
   won positions). Judge each network on `eval_gap.py`'s close subset first.
3. More user games into `matches/user_games/` and a rebuild: 61 close positions from three games
   is thin.
4. Profile the shipped engine again (Session 30's item 3), on an idle machine.
