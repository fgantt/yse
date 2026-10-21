# NNUE Implementation Session Log

## Session 36: Twice the even-game data

**Date**: 2026-10-17 to 2026-10-21
**Phase**: Phase 2w
**Objective**: Session 35's strongest lever, even-game data volume: double the corpus, retrain.

> **READ THIS FIRST** (state at the end of this session).
>
> 0. **Shipped 2026-10-21: `nnue/default.bin` is `q9`** (9 king buckets, 187.2M positions, epoch 1
>    of 2 from p9; p9 is kept as `nnue/default-p9.bin`). SPRT [0, 40] vs p9 (same binary):
>    **PASS, +173 ± 112, 36-1-13 after 25 pairs** (`matches/s36/sprt_q9.log`). See "Batch 3 and
>    q9" below, including why its first even-game numbers (76% / 24%) were wrong.
> 1. **Shipped first: `p9`** (9 king buckets, 125.0M even-game positions; the
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
> 5. **Even games vs YaneuraOu are only comparable within one run.** The same p9 scored 88%, 86%
>    and then 69% at 3k nodes; at night other processes loaded the Mac (load average 18 with 10
>    of our threads). Our side plays on a clock and YaneuraOu on nodes, so load only hurts us.
>    Run a control network side by side, or use the SPRT.
> 6. **Running at the end of the session**: `q9e3`, one more epoch of q9 (from its epoch-2
>    checkpoint) at lr 0.0003 (`weights/session36/train_q9e3.sh`), then SPRT vs q9.

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
- `matches/user_games/2026-10-20_4mai_win116_windows.txt`: the user's four-piece win with p9 on
  their Windows PC (2940e55) against YaneuraOu with the Suisho5 network, stronger than the
  halfkpvm build used for our gates (resigns at ply 116; mated from ply 106).

## p9e3: one more epoch (more passes vs more data)

One epoch of p9 at lr 0.0003 on the 125M pool (`train9b.sh`): held-out pearson 0.826. Even games,
run beside p9 at the same time: 3k 90.8% vs p9 86.3%, 10k 36.3% vs 35.0% (its first run, 71% /
12%, overlapped a 45 GB corpus load and is void). SPRT vs p9 stopped undecided at 200 games:
108-2-90, LLR +1.02 (`matches/s36/sprt_p9e3.log`). A small gain at most.

## Batch 3 and q9

Batch 3 (`corpus_s36b/`, seeds 370001-14, game ids from 3,000,000): 66.96M labelled positions,
62.27M new (93.0%), **187,234,110 pooled** (`corpus_s36b/pool_d8.jsonl`, 43 GB; the trainer holds
~45 GB). `q9` = p9 trained 2 more epochs at lr 0.001 on it (`next.sh`, seed 50), 5.2 h per epoch.
Both epochs scored r_clamp 0.816 on the new split, so the trainer kept epoch 1.

On the same held-out games (`eval_correlation`, 6M-line slices of the old pool and of batch 3,
~310k held-out positions each), pearson: p9 0.817 / 0.814, p9e3 0.819 / 0.816, q9 epoch 1
0.821 / 0.818, epoch 2 0.824 / 0.820.

Even games misled here. The first run (q9 epochs 1 and 2 together) gave 3k 75.8% / 74.6% and
10k 24.2% / 18.8%, apparently far below p9. A replay of q9 beside p9 under the same conditions
gave **q9 75.4%, p9 69.2%**: the machine was loaded by something else (see READ THIS FIRST 5).
The SPRT of q9 epoch 1 vs p9 (1 s, 4 threads each, seed 3603) then passed in 50 games. Shipped.
