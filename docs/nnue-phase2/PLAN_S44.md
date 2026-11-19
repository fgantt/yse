# PLAN S44: closing the gap to YaneuraOu + Suisho5 (speed, a real yardstick, two belief tests)

Started 2026-11-18 after the S43 ship (`80fcef5`) and an even-game loss on the user's PC. Council
(direction, `matches/s43/council_r10_brief.md`; note in RESEARCH_S40 last section). The user
approved all five steps; machine rule: the PC can be pushed hard, the Mac in hours, never days.
The engine is meant for public download: speedups must be portable (no `target-cpu=native`
assumptions; SIMD with a baseline path and run-time detection).

## 1. Where the user's games stand (2026-11-18)

- The user's opponent: YaneuraOu V9 AVX512VNNI with **Suisho5** (nn.bin md5 ac493583), not the
  halfkpvm network our guard uses. GUI defaults: 16 threads, 60 min + 10 s, our hash 256 MB.
- PC speed (10 s, one middlegame): YaneuraOu 1.47M nps on 1 thread, 14.3M on 16; ours 0.25M and
  2.4M: **5.9x**. Depth after 10 s: 25 vs 16.
- Loss analysis: `matches/user_games/2026-11-18_even_loss142_notes.txt` (ply 21 3d6d, −131 by
  deep YaneuraOu; ours chose it in 1 of 18 searches; hash 256 vs 1024 MB: no difference).

## 2. Profiles (step 1)

- Mac, 1 thread (`matches/s43/prof1t_top.txt`): NNUE `forward_from_activations` 20.6%, SEE ~15.5%,
  `is_square_attacked_by` 7.4%, attack-pattern lookups 9.3%.
- PC (Ryzen 7800X3D, Linux build of the same code in WSL, samply; `matches/s43/prof_pc1t_top.txt`,
  1-thread nps 254k = the Windows build's): NNUE forward **24.6%**, SEE ~16.7%,
  `is_square_attacked_by` 7.1%, magic lookups 4.0%, ordering sort 4.3%, `MoveTestFilter` 3.1%.
  (Profiling in WSL needs `perf_event_paranoid` 1, set once with sudo; it resets on restart.)

## 3. Yardstick vs YaneuraOu + Suisho5 (step 2, PC)

Ours (80fcef5 PC build) 4 threads, 1 s byoyomi, 256 MB, MoveOverhead 100, `--time-loss-ms 100`;
YaneuraOu + Suisho5 1 thread, 256 MB, no book, `go nodes N`; two games at once
(`matches/s43/yard/yard.sh`). Pilot, 40 games per rung, no time losses:

| YaneuraOu nodes | W-D-L | Score | Elo |
|---|---|---|---|
| 10k | 35-1-4 | 88.8% | +359 ± 85 |
| 30k | 29-0-11 | 72.5% | +168 ± 50 |
| 100k | 7-0-33 | 17.5% | −269 ± 68 |

Even at about 48k nodes (~30 ms of YaneuraOu on 1 thread against our 1 s on 4). The standing
yardstick is **50k nodes**; baseline for 80fcef5: 240 games (seeds 50200/50201), see §6.

## 4. Network test (step 3, Mac; Gemini's belief)

`80fcef5 --features yo-diagnostic` with halfkpvm (`usi-engine-80f-yo`, default path checked
identical to the shipped engine) in the guard's 16x row (1.6M vs YaneuraOu halfkpvm 100k, 240
games, seeds 43800-43811), paired with 80fcef5 + a1 (49.8%): halfkpvm **50.2%**, difference
**+3 ± 33** (95% −62 .. +68). Our network a1 is level with halfkpvm inside our search; the
PLAN_S41 Step 0a −101 (1.66 SE, older engine) did not hold. The network is not the next lever
against halfkpvm; Suisho5's margin over halfkpvm (~110-200, RESEARCH_S40 A2) still is a gap.

## 5. Speed (step 5): search-identical changes, worktree `s44` from `0669b2d`

Each verified with the fixed-node fingerprint (`s43_ident.py`, g1108 plies 30-69 and 70-109,
200k nodes, 1 thread: byte-identical to `engines/usi-engine-stable`) on the Mac, on the PC's
native x86 build and on a generic `-C target-cpu=x86-64` build. Speed: `matches/s43/speed/
speed_ab.py`, 1 thread, `go nodes 1000000` over 8 positions, builds alternating, 4 rounds,
median (Mac runs were taken with the network test loading the Mac; relative only).

| Change | Where | Mac | PC |
|---|---|---|---|
| SEE values from ordering reused by the search (`LeanHist::order_see`, `MoveSee`) | lean_hist.rs, lean.rs | +0.4% | |
| Hand counts in one pass (`CapturedPieces::counts`) in hashing | board.rs, zobrist.rs, search_engine.rs | +0.4% | +0.7% (both) |
| NNUE layer 2: blocks of 32 rows in 32-bit sums, then 64-bit; x86 16-bit multiply-add kernels (SSE2 always, AVX2 by run-time detection) | nnue.rs | +1.8% | **+11.6%** |
| Opponent attack map once per node for ordering (`BitboardBoard::attacked_squares`) | bitboards.rs, lean_hist.rs | +4.2% | |
| Ordering sort on one packed u64 key (same order) | lean_hist.rs | +1.7% | |
| SEE: no x-ray rescan for drops or squares off the target's lines | see_calculation.rs | +1.2% | |
| Legality and gives-check from per-node pins, discovered-check blockers and a king-danger map (`LineBlockers`, `safe_by_lines`, `check_by_lines`), positions with either side in check keep the full test | moves.rs, bitboards.rs | **+10.3%** | |
| **Total, shipped vs s44g** | | **+20.3%** | **+34.1%** native, +22.4% generic x86-64 |

- The NNUE finding: the 32-bit fast path never ran. The shipped network's largest layer-2 column
  sum is 659,019 and activations reach 4,096-16,383 (2M evaluations counted), so the whole-column
  bound always failed and every evaluation took the 64-bit path: a scalar loop on x86. Blocks of
  32 rows fit 32 bits; exact.
- Tests added: `layer_2_pair_kernels_match_i64` (x86), `layer_2_blocked_matches_i64`; the
  attack map against `is_square_attacked_by` on every square of 200 random games; the legality
  oracle (`evasion_filter_matches_full_test`) now compares every position, not only in-check ones,
  and also `gives_check` (seeds 41, 7, 1234, 99999: all ~1.85M comparisons agree; seeds 7 and
  99999 fall short only of the test's double-check count).
- Fixed: `popcount.rs` test did not compile on x86_64 (`hardware_duration` out of scope).
- Found, not fixed (a search change, needs a gate): `lmr_tactical_candidate` computes a capture's
  SEE on the board *after* the move is made (`lean.rs`, after `make_move_with_info`).
- Not portable to test here: the Mac has no Rosetta, so x86 tests run on the PC.

- PC speed (quiet PC, 2026-11-18 17:40, `matches/s43/speed/speed_s44g_pc.txt`): the shipped
  code 29.7 s, s44g native 22.1 s (**1.341x**), s44g built for generic x86-64
  (`-C target-cpu=x86-64`, the kind of binary a download would be) 24.3 s (1.224x). The generic
  build loses ~9% to native (no POPCNT/BMI etc. at compile time): per-ISA release builds, as
  YaneuraOu ships, are worth considering.

## 6. Yardstick baseline (80fcef5, 50k)

240 games (seeds 50200/50201): W-D-L 109-9-122, **47.3%, −19 ± 22** (95% −61 .. +24), halves
−3 ± 31 / −35 ± 31, no time losses, mean think 727 ms vs 33 ms. This is the number later builds
are compared with at the same setting.

## 7. Pending

- Transfer test (step 4, Opus's belief): 80fcef5 vs 4dfe9c4, 16 threads each, 4 s byoyomi,
  100 games, one at a time (~14 h, overnight; started 17:45); both from the yse-s43stack folder (network a1,
  unchanged between them; the yse-s40 tree has no nnue/default.bin).

## 8. Shipped (user's OK, 2026-11-18)

`2b99c36` "Speed: search-identical speedups" (the s44 worktree; its build is byte-identical to the
tested `usi-engine-s44g`), and `engines/usi-engine-stable` = that Mac build (md5 2f6fe495…).
The search is unchanged, so the gate does not apply; the hash default stays 256 MB (§1: 1024 MB
showed no difference at the user's settings). The yardstick baseline (§6) is 80fcef5's; the next
yardstick run measures the faster build at the same 50k setting.
