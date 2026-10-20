# NNUE Implementation Session Log

## Session 36: Twice the even-game data

**Date**: 2026-10-17 to 2026-10-20
**Phase**: Phase 2w
**Objective**: Session 35's strongest lever, even-game data volume: double the corpus, retrain.

> **READ THIS FIRST** (state at the end of this session).
>
> 1. **Shipped: `nnue/default.bin` is `p9`** (9 king buckets, 125.0M even-game positions; the
>    previous default k81 is `nnue/default-k81.bin`). SPRT [0, 40] vs stable (same binary, k81):
>    **PASS, +100 ± 68, 64-36 after 50 pairs** (`matches/s36/sprt_p9.log`).
> 2. **Even games vs YaneuraOu, same openings** (`matches/s36/even/`, seed 4201, 2 random plies,
>    us 4 threads 500 ms, YaneuraOu Threads=1 `go nodes N`; k81 re-run under identical settings):
>
>    | YaneuraOu nodes | k81 | **p9** | p81 |
>    |---|---|---|---|
>    | 3,000 | 76.7% | **88.3%** (pairs better/worse 21 / 9) | 87.5% (18 / 7) |
>    | 10,000 | 22.9% | **33.8%** (21 / 11) | 30.8% (19 / 12) |
>
>    p81 (81 buckets) also passed its SPRT (+77 ± 57, 86-1-55 after 71 pairs) but trails p9 in
>    both rungs and trains ~2.5x slower (9 h vs 3.5 h per epoch). Prefer 9 buckets while data grows.
> 3. **Data volume is still the lever, three times running**: 14M -> 62.6M -> 125M positions took
>    the 3k rung 60% -> 78% -> 88% and the 10k rung 11% -> 23-28% -> 34%.
> 4. **The user's goal is general strength, not handicap play** (restated 2026-10-17). Session 35's
>    handoff put handicap king-danger data next; that plan is dropped. Four-piece games are the
>    user's own gauge (8 -> 6 -> 4 pieces so far), not a target.
> 5. **Running at the end of the session**: batch 3 (`corpus_s36b/`, 584k more games, seeds
>    370001-14, game ids from 3,000,000) pooling into ~187M positions; and `p9e3`, one more epoch
>    of p9 at lr 0.0003 (more passes vs more data).

---

## Data (`corpus_s36/`)

Same recipe as `corpus_s35/gen`: YaneuraOu self-play, 4k vs 6k nodes, Threads=1, 8 random plies,
14 parallel `usi_match` runs of 41,686 games (`corpus_s36/gen.sh`, seeds 360001-14; none of the new
openings occur in the old set). 583,604 games in 6.4 h; labelled at YaneuraOu depth 8 by
`scripts/label_stream.py` (15 workers, 4.3 h; 67.0M positions). `scripts/label_stream.py` now takes
`GID_START` (here 2,000,000) so pooled corpora keep distinct game ids: the trainer's held-out split
is by game id. `corpus_s36/dedupe_pool.py` keeps every old record and each new position not already
present: 62.4M of 67.0M new positions kept (93.1%), **124,964,633 pooled** (`pool_d8.jsonl`, 29 GB;
the trainer holds it in ~16 GB).

## Training (`weights/session36/`)

Session 35 recipe (clamp 2000, scale 410, outcome 0.3, lr 0.001, Adam, 2 epochs), `p81` from k81
(`train.sh`) and `p9` from Session 35's k9 (`train9.sh`), run concurrently, one core each.
Held-out pearson (new split, not comparable to Session 35's 0.808/0.813 exactly): p9 0.823, p81
0.824 (the trainer kept p81's epoch 1 as best). The trainer prints no score before epoch 1.

## Gates

- Even games: table above. The k81 re-run (76.7% / 22.9%) agrees with Session 35's figures
  (78.3% / 27.5%) within noise; Session 35's logs did not record YaneuraOu's thread count.
- SPRT: 1 s/move, 4 threads each, `--sprt-elo0 0 --sprt-elo1 40`, seed 3601. `target/release/usi_match`
  was a September build without the SPRT flags; rebuilt.

## Other

- `matches/user_games/2026-10-18_4mai_win110.txt`: the user's four-piece win with stable + k81
  (YaneuraOu as White resigns at ply 110; mated from ply 100).
- Other USI engines for non-YaneuraOu opponents are installed outside the repo in
  `../shogi-engines` (Suisho5, Fairy-Stockfish + shogi NNUE, Gikou 2 built natively with sse2neon,
  Apery Rust); see its README.
