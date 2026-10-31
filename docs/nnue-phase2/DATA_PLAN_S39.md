# Training-data plan (Session 39 proposal, revised 2026-10-30)

**Status: Phase 0 done 2026-10-31 (`SESSION_LOG_039.md`).** Quiet filtering and mixing a Hao
sample into our pool shipped m8; Hao alone was weaker. Phase 1 below predates those results and
will be re-sized from them, with approval for each download.

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

### Phase 1: 1.5–2B positions (bigger, needs approval for each download)

1. **Published data, about 1–1.5B positions.**
   - Up to 200 Hao files (about 60 GB) if Phase 0's Hao check passes.
   - Also Suisho5 (185 GB as a whole) if the Hao check shows the stronger teacher is what
     matters.
   - Decode and quiet-filter, then pack: about 100 GB per 1B positions packed.
   - Disk: up to ~185 GB of archives plus ~200 GB packed, about 400 GB of the 839 GB free.
     Archives are deleted once packed, if the user agrees.
2. **Our own data: positions h4 steers into, about 150–200M.**
   - h4 against YaneuraOu at low node counts, from the start position and from published-set
     positions.
   - Neither teacher's self-play reaches the positions our engine chooses, and those are where it
     misjudges. Training on the positions the learner actually visits is the standard fix when a
     learner only sees its teacher's states.
   - **Labels:** depth 9 from Suisho5, which is already in `../shogi-engines`. That matches the
     published sets, if Phase 0 shows the stronger teacher helps. Otherwise YaneuraOu at depth 8,
     as now.
3. **Windows PC, from the start.** It generates and labels part of item 2 while the Mac decodes,
   packs and trains.
   - I'll prepare a self-contained package for you to unpack and run there (it can't be
     reached from here):
     - a Windows build of `usi-engine` with h4, and of `usi_match`;
     - the generation and labelling scripts, with their settings in one file;
     - a README.
   - It uses the PC's own YaneuraOu (the AVX512VNNI build). I'd need the PC's core count to set
     the number of parallel slots.
   - Output is JSONL batches. You copy them back, and I check and pool them.
   - The Mac does the same work at ~6M positions/hour. If the PC matches that, 200M takes about
     a day with both running.
4. **Train and gate.**
   - Train from scratch on the full mix, choosing the epoch by held-out loss. The held-out set
     must include games from each source.
   - Screen against h4, then the ship SPRT: 1 s, 4 threads, [0, 40].
   - Also check the YaneuraOu ladder rung and the eval-gap set from your games
     (`scripts/eval_gap.py`). These are diagnostics only, not targets.

Gate: ship if it passes, then weigh the next step: the rest of the Hao set, or more h4-steered
data. If 1.5–2B positions do not beat h4, data is not the limit either, and the next suspects are
the network's input features or search.

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

## Decisions needed from the user

1. **Approve Phase 0**, including the download of 50 files (about 15 GB) of `shogi_hao_depth9`
   from huggingface.co/datasets/nodchip/shogi_hao_depth9 (MIT licence). It runs on the Mac
   only, for about 1.5 days.
2. **The Windows PC's CPU and core count**, so the package can be sized. I'll prepare the
   package while Phase 0 runs.
3. **Later, after Phase 0:** approve the Phase 1 downloads (up to 200 more Hao files, about
   60 GB, and possibly Suisho5 at 185 GB) and the multi-day run.
