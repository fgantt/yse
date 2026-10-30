# NNUE Implementation Session Log

## Session 38: A GPU trainer, and the old trainer was the limit

**Date**: 2026-10-28 to 2026-10-29
**Phase**: Phase 2w
**Objective**: Session 37's direction: survey the field, then pick and check a plan. The plan
chosen was to rule out the trainer before buying more data.

> **READ THIS FIRST** (state at the end of this session, 2026-10-29).
>
> 0. **Shipped network: `nnue/default.bin` is h4** (2026-10-30): scratch2 epoch 4, the same recipe
>    as g5 with the rate decaying faster (x0.7 per epoch); **SPRT vs g5: PASS, +56.1 ± 44.7**
>    (115-2-83). g5 kept as `nnue/default-g5.bin`. Before it, **g5** (2026-10-29): trained **from scratch** by the
>    new GPU trainer (`trainer/`, PyTorch) on the same 375M depth-8 pool as t8
>    (`corpus_s37c/pool_d8.jsonl`, packed as `corpus_s38/packed_d8`), epoch 5 of
>    `trainer/runs/scratch1`. **SPRT vs s9: PASS, +93.3 ± 65.2** (76-2-44, 1 s, 4 threads each).
>    s9 kept as `nnue/default-s9.bin`. It is a **v5** file (`SHOGINNUE5`, per-layer shifts):
>    engines built before this session cannot load it and fall back to the classical evaluation.
>    `engines/usi-engine-stable` was rebuilt from this commit (old one: `usi-engine-stable-s9`).
> 1. **The old trainer was the bottleneck, not data volume.** `nnue-offline-trainer` rounds the
>    input layer to integers after every batch (median |w| = 4), so small updates vanish; it
>    also trains one epoch at a time, chained from the previous network. Session 37 read the
>    flat held-out pearson as "data running out of steam". Training the same data from scratch
>    in float beat t8 (pooled screens PASS, +31 and +42) and s9 (+93).
> 2. **Fine-tuning t8 in float did nothing** (screens -4, 0, -18 vs t8): the chained network sits
>    in a basin the float trainer does not leave. Start from scratch.
> 3. **Pick epochs by held-out loss, not pearson.** scratch1 epoch 3 (best pearson, 0.818) lost
>    to t8 (-34, FAIL); epoch 5 (lowest loss, 0.03598) won. t8's pearson (0.827) is higher than
>    either. The loss includes the 0.3 outcome term; pearson against the teacher alone misleads.
> 4. **It overfits after ~3 passes** of 355M positions (positions within a game are near
>    duplicates). scratch2 (lr 1e-3, x0.7 per epoch) reached a lower loss, 0.03562 at epoch 4;
>    it beat g5 (screens pooled PASS, ~+24 in the two full runs; SPRT +56) and shipped as h4. More data, now cheap to train on (an epoch
>    of 100M positions takes ~5 min), is the obvious next lever, together with longer games and
>    more varied openings.
> 5. **Other directions from the survey, still open:** mate-threat search near the king,
>    correction history, time management, SPSA tuning. The Windows PC (RTX 4070 Ti SUPER) is not
>    needed yet; the Mac GPU trains a network in ~1.5 h.

## What was built

- **`src/bin/nnue_pack.rs`**: `pack` (JSONL corpus -> 100-byte records, every 20th game held out,
  shuffled through bucket files; 375M positions in 3 min), `tables` (feature index tables from
  the engine's own functions), `check` (features and engine evaluations for `verify.py`).
- **`trainer/`**: `nnue_data.py` (records -> embedding-bag indices on the GPU), `nnue_model.py`
  (float network with the king factoriser, exact numpy emulation of the engine's integer
  forward pass, import/export), `train.py` (Adam, batch 16384, StepLR per 100M-position epoch,
  Session 35 target: 0.7 sigmoid(eval/410) + 0.3 outcome, MSE), `verify.py`. See
  `trainer/README.md`.
- **Engine: per-layer shifts** (`NNUEWeights::l2_shift`, `l3_shift`, v5 `.bin`, JSON
  `shifts`). A float network rounded with the fixed shift 6 lost 47 cp to rounding; the exporter
  now searches the hidden-layer scales and shifts and lands at ~1-3 cp mean |int - float|.
  Default (6, 6) networks still save as v4. `nnue-offline-trainer` refuses networks with other
  shifts.
- **`nnue-weight-diff`**: input-layer saturation and |w| percentiles.

Checks: features 0 mismatches over 2,000 positions; the integer emulation equals the engine's
evaluation on all 2,000, for t8 (v4) and a v5 GPU network; NNUE unit tests and
`nnue_incremental_games` pass.

## Results

| Network | Training | Held-out pearson / loss | Games |
|---|---|---|---|
| t8 (reference) | Rust trainer, chained | 0.8267 / 0.03737 | +28 vs s9 (S37, inconclusive) |
| ft_t8 e10 | t8 fine-tuned in float, lr 2e-4 x0.75 | 0.8293 / 0.03672 | vs t8: -4.3 ± 34.4, 0.0 ± 32.0 (FAIL x2) |
| ft_t8 e5 | same, epoch 5 | | vs t8: -17.7 ± 44.6 (FAIL) |
| scratch1 e3 | scratch, lr 1e-3 x0.95 | 0.8183 / 0.03620 | vs t8: -34.4 ± 52.8 (FAIL) |
| **scratch1 e5 = g5** | same, epoch 5 | 0.8161 / 0.03598 | vs t8: +30.8 ± 27.5 (600 cap), +41.8 ± 33.2 (PASS); pooled PASS LLR +5.25. **vs s9: +93.3 ± 65.2 (PASS)** |
| scratch1 e12 | same (stopped) | 0.8099 / 0.03645 | overfitting |
| **scratch2 e4 = h4** | scratch, lr 1e-3 x0.7 | 0.8214 / 0.03562 | vs g5: +24.9 ± 28.8, +23.8 ± 26.6 (600 cap), +124.5 ± 84.3 (PASS, 64 games); pooled PASS LLR +3.97. **vs g5: +56.1 ± 44.7 (PASS)** |

Screens: 4 threads each, 250 ms, SPRT [0, 40], three at a time (`matches/s38/screen*.sh`).
Ship gate: 1 s, 4 threads each, SPRT [0, 40], the same new binary on both sides (stable could not
read v5), `matches/s38/sprt_B5.sh`.

User games with g5 on the Windows PC vs YaneuraOu halfkpvm (2026-10-29): a two-piece handicap
win (`matches/user_games/2026-10-29_2mai_win102.txt`; s9 had lost the last two) and an even-game
loss (`2026-10-29_even_loss134*`): YaneuraOu had g5 at -200 to -530 from moves 16-22, after the
early bishop trade and the corner-lance plan, while g5 still scored itself about even.

Speed: one 3 s search, 1 thread: t8 536k nps, ft_t8 e10 485k nps. One sample each; the fast
16-bit layer-2 path applies only when the accumulators stay <= 32767, and the exporter keeps the
sample maximum at half that. Not yet measured properly; the games include whatever cost there is.
