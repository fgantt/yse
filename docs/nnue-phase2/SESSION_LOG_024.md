# NNUE Implementation Session Log

## Session 24: Modern Architecture — Two Perspectives, King Buckets, and a Trustworthy Bake-off

**Date**: 2026-09-18 – 2026-09-19
**Phase**: Phase 2r
**Objective**: Move toward a modern NNUE: layer 2 reading both perspectives, and features conditioned on
each side's own king square. Built in stages, each judged by bake-off.

> **READ THIS FIRST.** Every head-to-head bake-off before commit `83bcf6e` reused one engine per side
> for all games, so hash tables, history and caches carried over and the same network playing itself
> did not replay the same game. Those results carried extra noise (the old harness gave −34.9 ± 72.0
> for a network against itself). Use the paired harness: fresh engines per game, each opening played
> twice with colours swapped, CI over opening pairs.

---

## Architecture (commits `a6ed660`, `8f077c7`, `5edbea5`, `401b128`)

- **Two perspectives** (`--dual-perspective`): layer 2 reads the side to move's activated accumulator
  followed by the opponent's (256×2 → 32 → 1, one shared feature transformer). The engine already kept
  one accumulator per perspective, so it costs no extra updates. PSQT term `(stm − nstm) / 2`.
  `NNUEWeights::is_dual()` reads it from the layer-2 shape.
- **King-conditioned features** (`--king-features`): each perspective's POV features (board + hand)
  are repeated per own-king bucket; the bucket is the own king's square seen from its own side.
  `--king-buckets 81` (one per square, 189,945 rows) or `9` (3 file groups × home rank / second rank /
  further up, 21,105 rows). The engine reads the scheme from the row count. A king move, or a king
  capture inside quiescence, marks that perspective dirty and `evaluate_for` rebuilds it.
- **Warm starts**: `make_dual()` (zeroed opponent half) and `tile_for_king(n)` (copy into every
  bucket) leave the computed function unchanged, so each stage starts from the previous network.
- **Factoriser** (`--king-factorizer`): each king-conditioned weight = shared per-feature mean + a
  per-bucket difference; the shared part gets the summed gradient of all buckets; the sum is what is
  saved. `--king-delta-decay` applies decoupled decay to the differences (pulls buckets to shared).
- Weight files are v4 (`SHOGINNUE4`) with the full feature set; v2/v3 still load.
- Tests: expansions and the factoriser split preserve the network; trainer feature lists match the
  engine refresh for both bucket schemes; a random king+dual network is exact at every searched node
  (king moves included), colour symmetric, never stale.

## Bake-off harness (commit `83bcf6e`)

`elo-tester` now builds fresh engines for every game, plays each random opening twice with colours
swapped, and reports a 95% CI over opening pairs. Same network against itself: every pair replays
identically, paired Elo exactly 0.

## Results (4.24M-position corpus, 200 games = 100 opening pairs, fixed depth 2)

Opponent in every row: `s24_lr01` (single perspective, frozen material PSQT, the current default).

| network | held-out r (best epoch) | paired Elo vs `s24_lr01` |
|---|---|---|
| two perspectives (`dual`) | 0.802 (5) | −1.7 ± 48.8 |
| + 81 king buckets | 0.798 (1) | −77.7 ± 49.5 |
| + 81 buckets + factoriser, lr 0.005 | 0.799 (1) | −59.6 ± 44.9 |
| + 9 buckets + factoriser | 0.801 (1) | — |
| + 9 buckets + factoriser + delta decay 1e-3 | 0.802 (1, flat to 3) | −33.1 ± 46.8 |

Re-check of the data gain on the paired harness: `s24_lr01` vs `big_frozen` **+66.8 ± 50.0**.

**Diagnosis: the king variants overfit.** Training error falls to 0.09–0.10 by epoch 4 while held-out r
declines from epoch 1; the two-perspective network holds 0.802 at training error 0.12. Nine buckets
still memorise. Delta decay stops the decline (training error 0.118 → 0.114 over 4 epochs, held-out flat
at 0.802) but does not add strength. 64K games, with strongly correlated positions within a game, is too
little data for king conditioning; production NNUEs train on billions of positions.

## In progress

Another 24,000 games (`corpus_s25/run.sh`, ids from 300,000): `nnue_corpus_s25_combined.jsonl`,
**6,790,401 positions, 64,500 games**, audit clean (0 chain breaks; 0 move-list differences from the
teacher on 679,041 positions). Retraining all three designs on it (`weights/session26/`): single
perspective, two perspectives, and regularised 9 buckets, to see which gains from more data.
