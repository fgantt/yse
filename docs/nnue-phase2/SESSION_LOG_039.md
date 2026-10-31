# NNUE Implementation Session Log

## Session 39: Quiet positions and published data

**Date**: 2026-10-30 to 2026-10-31
**Phase**: Phase 2w
**Objective**: Phase 0 of the training-data plan (`DATA_PLAN_S39.md`): cheap checks of volume,
a quiet-position filter, and a published teacher, before any multi-day data run.

> **READ THIS FIRST** (state at the end of this session, 2026-10-31).
>
> 0. **Shipped network: `nnue/default.bin` is m8** (2026-10-31): our quiet-filtered pool mixed
>    with a quiet-filtered 50-file sample of nodchip's `shogi_hao_depth9` set, 452M positions,
>    h4's recipe, epoch 8 of `p1_mix_q`. **SPRT vs h4: PASS, +240.8 ± 131.7** (32-0-8, 1 s,
>    4 threads), stopped at 40 games, so the estimate is high; screens put it ~+50 over the quiet
>    network, which passed the gate at +64 over h4. Read it as roughly +100-150. h4 kept as
>    `nnue/default-h4.bin`. Same v5 format; `engines/usi-engine-stable` loads it unchanged.
> 1. **Volume still pays, a lot.** Half our pool (178M, chosen by game) lost to h4 by -89 and -168.
> 2. **Train only on quiet positions.** Dropping positions in check or where the next move
>    captures (37% of the pool) beat h4 with less data: screens +80 and +47, ship SPRT
>    **+63.5 ± 49.1** (111-0-77). The evaluation is only asked about quiet positions in search
>    (quiescence resolves the rest), so the others were noise.
> 3. **Hao alone is worse than our data; Hao added to ours is better than either.** 50 Hao
>    files (389M positions, Hao at depth 9) trained alone lost to h4 (-125, -147); quiet-filtered,
>    to the quiet network (-135, -89); quiet and with |score| > 3000 dropped, too (-147, -82). Its
>    scores match our teacher's scale (slope 0.99, pearson 0.93 on 1,000 positions; same best
>    move 51%), so the labels are not the problem: its self-play positions alone are less useful
>    to us. Mixed half and half with our quiet pool it beat the quiet network (+151, +49).
>    One sample and one recipe; the user does not accept "published data is worse" from this.
> 4. **Longer training on the mix gave little.** 16 epochs at x0.85 (best at epoch 16, held-out
>    loss 0.02985 vs 0.02988) screened +3 (FAIL) and +57 (PASS) vs m8, about +19 pooled over
>    764 games. More data, not more passes.
> 5. **Next (Phase 1, needs approval):** grow both kinds. More Hao files (each ~290 MB, ~7.5M
>    positions) and our own generated games, quiet-filtered and mixed; the user wants our own
>    data doubled, proposed with its cost first. The Windows PC trains and can generate.

## Machines

- The **Windows PC** (Ryzen 7 7800X3D, RTX 4070 Ti SUPER) is now a second trainer: the same
  `trainer/` on CUDA in WSL, ~190k positions/s (the Mac's MPS ~320k). The Mac reaches it over
  Tailscale SSH; logins open in WSL, so `ssh host cmd` and scp fail and commands and files go
  through stdin. Jobs must run in the foreground of an SSH session the Mac holds open:
  processes detached with `setsid nohup` stalled when the session closed. A build copy of the
  repository lives at `C:\Users\fgant\yse-work`, apart from the user's checkout.
- The PC's "YaneuraOu" runs the Suisho5 network (its `eval\nn.bin` is byte-identical to
  `shogi-engines/yaneuraou-suisho5/eval/nn.bin`).

## What was built

- **`nnue_pack pack --half --quiet`**: half the pool by game id (held-out games all kept), and
  the quiet filter, which reads the next record of the same game to see whether the move played
  captures (the mover's hand grows).
- **`nnue_pack pack-psv`, `psv-check`**: our own reader for YaneuraOu's 40-byte PackedSfenValue
  (Huffman-coded board, score from the side to move's view, move, ply, result), written from the
  format description. 200,000 decoded positions all legal with every stored move legal; the
  quiet filter uses the stored best move. The last input file is held out.
- **`trainer/mix.py`**: interleaves packed directories at random in proportion to their
  (filtered) sizes, so the trainer's sequential reads see every source throughout; `--max-eval`
  drops lopsided positions.
- **`docs/nnue-phase2/hao/`**: the 50 Hao files used (name, size, sha256; spread over the set's
  8 generation runs) and the download script.

## Results

All screens 250 ms, 4 threads each, SPRT [0, 40]; ship gates 1 s. Logs in `matches/s39/`.

| Network | Data (train positions) | Best epoch | vs | Result |
|---|---|---|---|---|
| p0_half | our pool, half the games (178M) | 2 | h4 | -89, -168 (FAIL x2) |
| p0_quiet | our pool, quiet (223M) | 5 | h4 | +80, +47 (PASS x2); **ship SPRT +63.5 ± 49.1 PASS** |
| p0_hao | Hao sample (389M) | 4 | h4 | -125, -147 (FAIL x2) |
| p0_hao_quiet | Hao, quiet (230M) | 4 | p0_quiet | -135, -89 (FAIL x2) |
| p0_hao_q3000 | Hao, quiet, \|score\| ≤ 3000 (206M) | 4 | p0_quiet | -147, -82 (FAIL x2) |
| **p1_mix_q (m8)** | ours quiet + Hao quiet (452M) | 8 (last) | p0_quiet | +151, +49 (PASS x2); **ship SPRT vs h4 +240.8 ± 131.7 PASS** |
| p1_mix_q16 | the same, 16 epochs, x0.85 | 16 | m8 | +3 (FAIL), +57 (PASS); ~+19 pooled |

Held-out losses are not comparable across rows: each data set has its own validation games.

## Mistakes

- The first Hao-vs-YaneuraOu comparison (8 worker processes) hung for hours with its workers
  gone; a sequential version with an engine restart on failure finished in minutes.
- A PC run's end sends no notification; the capped-Hao screens started 45 minutes late.
  Every PC job now has a Mac-side watcher.
