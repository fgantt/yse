# Training-data plan (Session 39 proposal, revised 2026-10-30)

**Status: Phase 0 done 2026-10-31 (`SESSION_LOG_039.md`).** Quiet filtering and mixing a Hao
sample into our pool shipped m8; Hao alone was weaker. Phase 1 below was rewritten from those
results on 2026-10-31 and approved (Stages A and B) the same day. Stage A shipped a1
(2026-11-01, +~60 vs m8); Stage B is generating.

Approved 2026-10-30. The user had no revisions beyond the two below and said to use the
Windows PC however makes sense.

Revision 2 follows the user's answers:

- Other teams' published training data may be used when its licence allows and it is valuable.
  The no-copying rule is about code.
- Phase 1 is bigger.
- The Windows PC takes part from the start.

## Machines (2026-10-30)

- **Mac (caesar):** M4 Max, 16 cores, 64 GB memory; trainer on MPS.
- **Windows PC (cicero):**
  - Ryzen 7 7800X3D (8 cores / 16 threads), 31 GB memory (23 GB in WSL), 1.5 TB free.
  - RTX 4070 Ti SUPER: the same trainer runs on CUDA in WSL (`~/yse/trainer`, torch 2.14).
  - The Mac reaches it over Tailscale SSH. Logins open in WSL, and `ssh host cmd` and scp do
    not work, so commands and files go through the shell's stdin (`pc.sh`).
  - Long jobs must run in the foreground of an SSH session the Mac holds open. Processes
    detached with `setsid nohup` stalled once the session closed.
  - A build copy separate from the user's checkout: `C:\Users\fgant\yse-work`.
- **The PC's "YaneuraOu" runs the Suisho5 network.** Its `eval\nn.bin` is byte-identical to
  `shogi-engines/yaneuraou-suisho5/eval/nn.bin`. So the user's PC games were against Suisho5,
  and Suisho5 is there to label with.

## Why data, and why now

- Session 38 found that the Rust trainer, not data volume, caused the Session 37 plateau. The GPU
  trainer then gained +93 (g5) and +56 (h4) on the *same* 375M positions.
- On that data, every change to the training recipe and network shape now lands within about
  ±15 Elo of h4:
  - game-result weight 0.5 and 0.0;
  - faster learning-rate decay;
  - a middle layer, with and without the game-result term.
- The GPU trainer overfits after about 3 passes. With more positions it can keep learning, and
  an epoch of 100M positions takes about 5 minutes. Training on 1.5–2B positions is a few hours.
- **The weakness shows up in every recent loss.** All three even-game losses (YaneuraOu twice,
  Gikou 2 once) show h4 rating mutual-attack positions 300–900 cp too favourably for itself.
  Session 37 measured our teacher's depth-8 labels flattering the side to move by +207 cp in
  exactly those stretches. Some of h4's optimism may be learned from its labels, so the
  **quality of the teacher** matters as much as the volume.

## What we have

| | Now |
|---|---|
| Positions | 375M (about 3.3M games), deduplicated |
| Games | YaneuraOu vs itself, 4k vs 6k nodes, from the start position plus 8 random plies |
| Labels | YaneuraOu (halfkpvm) depth 8; game result from the same low-node games |
| Filtering | None: positions in check and in the middle of capture sequences are kept |
| Cost on the Mac | ~6M positions/hour (generation ~6.5 h + labelling ~4.3 h per 63M) |
| Disk | 839 GB free; JSONL ~230 B/position, packed 100 B/position |

## What the field does, and what is available

- **Stockfish (nnue-pytorch).** Trains on billions of positions and skips positions where the
  side to move is in check or the best move is a capture (`--smart-fen-skipping`). The network
  is only asked about quiet positions.
- **Suisho / Hao / YaneuraOu-family networks.** Published sets run to billions of positions,
  labelled at depth 9. Their games start from a book or Floodgate positions, with random and
  random-MultiPV moves added.
- **Published sets we may now use** (all from nodchip on Hugging Face, MIT licence, YaneuraOu's
  PackedSfenValue format: 40 bytes per position, with board, score, best move, ply and game
  result):

  | Set | Size | Teacher | Notes |
  |---|---|---|---|
  | `shogi_hao_depth9` | 320 GB, about 8B positions | Hao, depth 9 | ~300 MB files (~7.5M positions each), so a subset is easy to take. Not shuffled; positions not reduced to quiet ones. |
  | `shogi_suisho5_depth9` | 185 GB (7z volumes) | Suisho5, depth 9 | Shuffled. Each position is already the leaf of its quiescence search, so it is quiet. A multi-volume archive, so it comes as a whole. |
  | `shogi_suisho5_depth9_entering_king` | ~500M positions | Suisho5, depth 9 | Starts from 2015–2024 Floodgate games, focused on entering-king positions. |

  Hao and Suisho5 are much stronger teachers than YaneuraOu's halfkpvm network at depth 8. That
  difference bears directly on the optimism in h4's losses.
- **Floodgate.** Public game records, one 7z file per year since 2008, about 100 MB a year, with
  no stated terms. They are now only a fallback for variety, because the published sets already
  draw on them.

## Plan

### Phase 0: cheap checks (about 1.5 days, one ~15 GB download)

Each check trains with h4's recipe (lr 1e-3, x0.7 per epoch, lowest held-out loss) and is screened
against h4. They run one at a time on the GPU, with screens overlapping the next training run.

1. **Does volume still pay?** Train on half our pool, chosen by game id, and screen it against h4.
   Half losing by ~30 Elo or more means more positions should gain.
2. **Quiet-position filter.** Skip positions in check, or where the next move in the game is a
   capture. Train and screen.
3. **Teacher check with a Hao sample.**
   - **Download:** 50 files of `shogi_hao_depth9`, about 15 GB, about 375M positions.
   - **Reader:** write our own PackedSfenValue reader in `nnue_pack`. We write it ourselves from
     the format description.
   - **Check the reader:**
     - every decoded position is legal and has both kings;
     - every stored best move is legal;
     - our YaneuraOu at depth 8 agrees closely with the stored scores on 10k positions, which
       checks both the decoding and the score scale.
   - **Train** on the Hao sample alone (the same size as our pool), with the stored best move
     giving an exact quiet filter. Screen it against h4.
   - Hao-only at least level with h4 means it is the better teacher, and Phase 1 leans on it.

Gate: report the three results. The mix and size for Phase 1 follow from them.

### Phase 1 (revised 2026-10-31 from Phase 0's results; approved)

What Phase 0 showed (`SESSION_LOG_039.md`):
- Volume pays: half the pool lost 90–170 Elo.
- Quiet positions only: +64 at the ship gate.
- Hao alone lost ~100 to our data. Mixed half and half with ours, it shipped as m8, so variety
  pays.
- 16 passes instead of 8 over the same 452M gained ~nothing. The next gain has to come from new
  positions.

So Phase 1 grows both sources, quiet-filters everything, and judges each step by mixing it into
the current pool. All gates are against m8.

**Stage A: more Hao (first; cheap, about one day).**
- **Download:** 150 more `shogi_hao_depth9` files, about 44 GB (~1.1B positions, ~690M after
  the quiet filter), spread over the set's 8 generation runs like the first 50. The PC fetched
  15 GB in 17 minutes, so about an hour, checked against each file's sha256.
- **Pack** on the PC with `pack-psv --quiet`: about 69 GB.
- **Train two mixes**, each with h4's recipe, the epoch chosen by held-out loss and the
  validation set drawn from both sources:
  - A1: our quiet pool (223M) + all 200 Hao files quiet (~920M), about 1.14B. 2 passes is about
    23 epochs: ~3.5 h on the PC, ~2 h on the Mac.
  - A2: the same, with Hao subsampled to 2:1 (about 670M).

  If A1 beats A2, more Hao is still paying and the rest of the set is worth fetching later.
- **Screens** vs m8, then the ship SPRT for the winner.
- **CPU:** screens only. Downloading, packing and training run on the PC.

**Stage B: double our own data (the long part, about 2–3 days with both machines).**
- **Target:** ~375M new raw positions, ~235M after the quiet filter, which doubles the quiet
  pool.
- **Two kinds of games**, so the new data adds variety rather than repeats:
  - B1, about two-thirds: YaneuraOu self-play with the batch 3–6 recipe (4k vs 6k nodes, 8
    random plies), new seeds.
  - B2, about one-third: m8 against YaneuraOu, so the data covers the positions our engine
    steers into. Its cost per game is unmeasured: a 3,000-game test chunk first, then sized.
- **Labels:** YaneuraOu (halfkpvm) at depth 8, as for the existing pool, so the two pools stay
  one teacher. The PC's YaneuraOu runs Suisho5; it would get a copy of the halfkpvm network for
  labelling. That counts as using YaneuraOu as a labeller, which is allowed; nothing goes into
  our code or networks.
- **Cost:**
  - Mac: ~5.5M positions/hour with all 14 slots (batches 5–6: ~11 h per 62M). That's about
    68 h alone.
  - PC: ~8 cores, est. ~2.5–3M/hour, unmeasured until a first batch.
  - Together: about 45 h for B1 + B2, in 62M-position batches with fresh engines per chunk, as
    before.
- **The cost to you:** the Mac's cores are busy for about two days, and GUI games would compete
  with it. It runs in windows you choose; it can pause between batches, which are ~11 h each.
- **Disk:** ~85 GB of labelled JSONL plus ~24 GB packed on the Mac (749 GB free).

**Stage C: the full mix (after A and B).**
- Our quiet pool, about 460M, plus Hao quiet, about 920M (or Stage A's better ratio): ~1.4B
  positions, ~140 GB packed.
- 2 passes is about 28 epochs: ~2.5 h on the Mac, ~4 h on the PC.
- Screens and the ship SPRT vs the Stage A winner.
- Also the YaneuraOu ladder rung and the eval-gap set from your games, as diagnostics only.

**Order:** Stage A starts right away and finishes within a day. Stage B's first batch can run
alongside it, so its speed on each machine is measured early. The rest of Stage B runs in
windows you choose.

**Not in Phase 1:**
- The Suisho5 depth-9 set (185 GB, one multi-volume archive). Only if Stage A shows more
  published data keeps paying.
- Depth-9 relabelling of our pool. Session 37's depth-10 labels gained nothing.

### Operations (from earlier sessions)

- Fresh engine processes per 3,000-game chunk and per labelling batch: long-lived ones slow down
  about 4x.
- Run `caffeinate -u` while you are away; without it macOS throttles the run to 3–20% speed.
- Generation competes with your games on the GUI. Long runs go in windows you pick.
- Downloads go into their own empty directory and are read only by our own code (`python3 -I` or
  `nnue_pack`), never executed. Check sizes and file counts after each download.

## Risks and how each is caught

- **A decoding mistake would quietly poison training.** Caught by the legality, best-move and
  score-agreement checks before any training.
- **The published sets' score scale may differ from ours** (Hao and Suisho5 units against
  YaneuraOu's). Measured on the 10k overlap. If needed, give each source its own scale factor
  inside the target sigmoid.
- **Game results in the published sets come from their own self-play.** The game-result weight
  stays at 0.3 unless Phase 0 or 1 says otherwise. Weight 0 was +12 to +16 on our data, which
  was not significant.
- **Mixing teachers can blur the targets.** Compare the mix against Hao alone in Phase 1, using
  the screen.
- **Self-play Elo overstates real gains.** Watch the YaneuraOu ladder rung and your games, not
  only the SPRT.

## Decisions needed from the user (Phase 1)

1. **Stage A download:** 150 more files (about 44 GB) of `shogi_hao_depth9` from
   huggingface.co/datasets/nodchip/shogi_hao_depth9 (MIT licence), to the PC.
2. **Stage B generation:** about two days of the Mac's cores (plus the PC's), in windows you
   choose, and putting the halfkpvm network on the PC for labelling.
