# PLAN S45: depth or evaluation? (2026-11-19 night)

After the S44 speed ship, a first even win (Mac, vs YaneuraOu halfkpvm) and two losses (PC vs
Suisho5, Mac vs halfkpvm). Council r11 (`matches/s44/council_r11_brief.md`, note in RESEARCH_S40).
The user: "speed bought us depth and the first win"; be sure the focus is right.

## 1. Test 1: 4x time (PC, 16 threads, 360 s, fresh process)

None of the three decisive choices changed (PC 55 7g6f, Mac 77 2d2c+, Mac 79 6f6e); move 79 kept
score 0 at depth 23, 1.36G nodes. Files: `t4x_*.jsonl` in `/mnt/c/Users/fgant/s44pcloss` and
`s44macloss`.

## 2. The score 0 and the 30 s re-search limit (PC, `yse-s45diag`, worktree `s45diag`)

Diagnostic switches, off by default (fingerprint identical to 2b99c36 with them off):
`SHOGI_DIAG_REP=1` (no main-table store / correction learning of a 0 whose subtree met a
repetition), `=2` (any such result), `SHOGI_DIAG_NO_ASP30=1` (no 30 s per-iteration re-search
limit, `search_engine.rs:16514`). One 90 s run per setting, 16 threads (`matches/s44/pcloss/
diag_run.out`): the 0 survives `REP=1` at moves 77 and 79 (so not the table); 6h6g appears only
twice (79 with REP=1 + no limit at depths 16-17; 77 with no limit: depth 18 6h6g −120, yet
bestmove 3i4h). Single 16-thread runs are noisy (the unchanged build played 6f6e at 79 once and
3i4h twice). From the bare SFEN (no game history) the 0 persists: it comes from the search.
Several runs end with a last-depth fail-low that is never re-searched (time).

## 3. Network vs search on the same positions (Mac, 16 threads, ~90 s, `evalsep.out`)

Our search with a1 (2b99c36) vs with halfkpvm (`usi-engine-80f-yo`, same search):

| Position | a1: move, scores by depth | halfkpvm: move, scores | Suisho5 (deep) |
|---|---|---|---|
| Mac 79 | 3i4h, 0 at every depth 14-21 | 3i4h, 0 → −162 → +129 (6h6g d16) → −348, −336 | 6h6g −200..−344 |
| Mac 77 | 2d2c+, 0 at every depth 14-21 | **6h6g**, −35 .. −230 | 6h6g −151..−165 |
| Mac 71 | 2g2d, +436 → 0 | 2g2d, −80 .. −244 | 2g2d −198 |
| Mac 63 | 5h6h, +54 .. +254 | 5h6h, −113 .. −263 | 5h6h −144 |
| PC 55 | 7g6f, −62 .. +98 | 7g6f, 0 .. +51 | 6g6f −30 (7g6f −82) |

The zeros are ordinary minimax under an optimistic evaluation: a1 rates White's non-repeating
lines as bad for White, so White "takes" the repetition draw. With halfkpvm the same search sees
the danger and, at 77, plays Suisho5's move.

## 4. Survey: every Black move in 10 even games (Mac, 1 thread, 2M nodes each, `matches/s45/survey`)

Reference Suisho5 (FV_SCALE 24); 414 open positions (|Suisho5| < 1500). Error = engine − Suisho5:

| Suisho5 bucket | n | a1 bias | a1 >300 off | halfkpvm bias | halfkpvm >300 off |
|---|---|---|---|---|---|
| all | 414 | +204 | 24% | +131 | 20% |
| we are worse (< −300) | 158 | **+387** | 53% | +219 | 32% |
| −300 .. −100 | 78 | +172 | 6% | +152 | 18% |
| level | 132 | +59 | 2% | +47 | 5% |
| +100 .. +300 | 26 | +121 | 19% | +65 | 27% |
| better (> +300) | 20 | −56 | 15% | −14 | 20% |

Best move equal to Suisho5's: a1 56%, halfkpvm 57%. Both networks are optimistic when behind;
a1 clearly more (+387 vs +219). Caveat: positions come from games our engine (with a1) steered, so
they are selected for a1's misjudgments; scales differ somewhat between engines.

## 5. Speed

- Mac profile (macOS `sample`, 1 thread, 2b99c36): layer 2 is ~27% of search CPU.
- Layer-1 activations: 69% zero, 48% of row pairs both zero, 0% of 32-row blocks all zero.
- Worktree `s45speed`: non-zero rows/pairs listed without branches, then a branch-free multiply
  loop (NEON rows, SSE2/AVX2 pairs). Fingerprint identical on both machines (0 of 40 positions
  differ). Speed: **Mac +6.2%**, PC 0.99x (no gain). Not shipped.
- PC profile not taken (needs sudo in WSL; no answer overnight).

## 6. Value of a time doubling at 16 threads (PC, overnight)

2b99c36 vs itself, A 3 s vs B 1.5 s byoyomi, 16 threads each, 256 MB, 100 games, seed 50400,
2026-11-19 22:34 to 11-20 05:45, `/mnt/c/Users/fgant/s45dbl/d_s50400.csv` (copy in
`matches/s45/dbl/`). W-D-L **77-7-16, 80.5%, +246 ± 38** paired (95% +173 .. +320), halves 39-3-8 /
38-4-8, no time losses (longest think A 3029 ms, B 1487 ms; mean 2355 / 1193 ms, ratio 1.97).

- Depth is not saturating at the user's thread count: one doubling is worth about +250 in
  self-play at 16 threads, 1.5 -> 3 s.
- Against YaneuraOu, gains have run about half of self-play (`self_play_gains_shrink_with_depth`);
  the yardstick gave +74 for 0.42 doublings (§7a of PLAN_S44, ~+175 per doubling at 4 threads, 1 s).
- Set against the network: a1 ~ halfkpvm inside our search (PLAN_S44 §4, +3 ± 33); Suisho5 is
  ~110-200 above halfkpvm (RESEARCH_S40 A2). Closing the whole network gap is worth about one
  doubling; YaneuraOu's nps lead on the PC is still ~4.4x (5.9x / 1.34), about two doublings.
- Conclusion: speed stays the main lever. The optimism when behind (§3, §4) is a real, specific
  defect, but it is the second lever, and earlier attempts at the network (Suisho5 teacher, N4)
  gained ~0.

## 7. Shipped (user's OK, 2026-11-20)

`0425008` "Speed: branch-free sparse NNUE layer 2" (the `s45speed` kernel, §5), and
`engines/usi-engine-stable` = that Mac build (md5 7b9099e4…; the previous stable kept as
`engines/usi-engine-2b99c36`). Fingerprint on the shipped build: 0 of 40 positions differ from
2b99c36 (g1108 plies 30-69, 200k nodes). Search unchanged, so no gate. On the PC the kernel is
neutral (0.99x); the PC's speed work waits on a profile.

## 8. PC profile of the shipped code (2026-11-20)

Linux build of 0425008's source in WSL (`~/yse-s45prof`, native), samply, one middlegame, 20 s
(`matches/s45/pcprof/top1t.txt`, `top16t.txt`; same script and position as PLAN_S44 §2).
1-thread nps **339k** (S43 profile: 254k), 16 threads 3.07M.

| Area | 1 thread | Largest items |
|---|---|---|
| NNUE | 27.2% | layer 2 16.7% (AVX2 pairs kernel, inlined), forward 4.7%, make_move 3.3%, refresh 1.6% |
| SEE | 23.2% | `attackers_of` 8.8%, `add_sliders` 6.3%, `swap_value_from` 2.9%, `see_quiet` 2.9%, `order_see` 2.2% |
| Legality and move generation | 20.0% | `MoveTestFilter::test` 5.5%, per-piece generation ~4.3%, attack patterns 1.8% |
| Hashing and correction keys | 6.0% | `lean_corr::keys` 3.4%, `hand_hash` 2.3% |
| Ordering | 3.8% | `quiet_score` 2.0%, sort 1.8% |

16 threads: the same order (layer 2 17.3%, `attackers_of` 7.7%, `add_sliders` 6.9%). On the PC
layer 2 is already a vector kernel, hence no gain from §5; SEE is the largest part not yet
reworked: a list of (square, piece) pairs rebuilt per target square, copied per move and searched
linearly, where a bitboard exchange (attackers as one bitboard, least valuable by piece-type
masks) is the usual form.

## 9. Full games at the user's settings (PC, from 2026-11-20)

The user: why not play full games as on the PC and study our decisions against YaneuraOu? Short
tests measure Elo (300-500 games); full games (~2.5-3 h each, one at a time) cannot, but they are
the only place the long-time-control mechanisms act: the 30 s re-search limit, time allocation
(remaining/30, no extension on instability), the hash filling at 100M+ nodes per move.

- Harness: `usi_match --main-ms N --byoyomi B` (running clocks, `go btime .. wtime .. byoyomi ..`,
  as the GUI sends; `ysu/src/components/GamePage.tsx:1672`), `--move-log` (per move: think time,
  clocks, every info line of the mover). `src/bin/usi_match.rs`, `src/usi_client.rs` (`info_lines`).
- Run `f1` (`matches/s45/full/`, PC `/mnt/c/Users/fgant/s45full/`): ours (0425008 code, Windows
  build `yse-s45clock`, md5 183a2fea…) vs YaneuraOu V9 AVX512VNNI + Suisho5 (FV_SCALE 24), both 16
  threads, 256 MB, otherwise engine defaults; 60 min + 10 s byoyomi; 2 random plies then free play;
  up to 20 games, seed 50510; started 2026-11-20 07:44. Stop on request when the user wants the PC.
- After each game: Suisho5 judge curve, the decisive moves, each classed as evaluation / depth /
  time management / hash, from our logged depth, score, PV and time. Our engine does not report
  `hashfull`, so the hash class needs a replay: the decisive positions re-searched for the logged
  time at 256 vs 1024 MB.

## 10. Speed round 3 (worktree `s45see`, Mac; PC check pending)

Search-identical; fixed-node fingerprint 0 of 80 positions differ from stable (g1108 plies 30-109,
200k nodes); `cargo test --release --lib` 1170 passed. Mac speed (`speed_ab.py`, 8 positions x 1M
nodes, 4 rounds, same node counts) vs stable (0425008):

| Change | Where | Mac |
|---|---|---|
| Exchange evaluation on bitboards (`Exchange`): step attackers as one bitboard, sliders in small per-type queues in the order found, x-rays recomputed for the line's ray kind only; picks exactly the list version's piece (test `exchange_matches_attack_list` against it, with and without the magic table; fails on a wrong tie order or a missed x-ray) | see_calculation.rs | +6.2% |
| Layer-2 non-zero list per 32-row block (64 bytes) instead of 512 entries: the §5 kernel cleared 1 KB per block, 16 blocks per evaluation (~6% of samples in `memset`) | nnue.rs | +6.0% |
| Activations not zeroed before being written in full (`evaluate_dual`, 2 KB) | nnue.rs | +1.1% |
| **Total** | | **+14.3%** |

PC (2026-11-20, between full games; `matches/s45/pc_check_see.sh`, `/mnt/c/Users/fgant/s45seecheck`):
fingerprint 0 of 40 differ, native and generic x86-64 builds; x86 unit tests 1170 passed, 1 failed
(`test_memory_breakdown`: RSS is not read on Windows, file unchanged, not this change); the SEE and
layer-2 tests pass in the generic build too (runtime AVX2 dispatch). Speed vs the shipped 0425008
native build (8 positions x 1M nodes, 4 rounds): **native +7.0%, generic x86-64 +6.2%**.

## 10a. Shipped (user's OK, 2026-11-20)

`3eec8b3` (§10), and `engines/usi-engine-stable` = that Mac build (md5 6fd0701a…; fingerprint
re-run on this binary: 0 of 80 differ). The previous stable is kept as `engines/usi-engine-0425008`.
The PC full games keep the 0425008 build (`yse-s45clock`) until the run ends, so games stay
comparable; the search is identical either way.

## 11. Full game 1 (run f1, 2026-11-20 07:44-09:07): loss in 114 plies, ours Black

Files: `matches/s45/full/f1.csv`, `f1.jsonl` (per move), `judge/f1_judge.jsonl` (Suisho5 on the Mac,
8 threads, MultiPV 4 for 60 s, each candidate alone for 30 s; scores for Black).

- YaneuraOu's in-game view (Suisho5, depth 27-40): level to ply 36 (−58 .. +32), then −117 (38),
  −217 (40), −417 (46), −666 (54), lost. Ours stayed near level: +31 at ply 51 where YaneuraOu
  said −590 (the optimism of §4).
- Depth and time: plies 41-80, ours 52 s per move for depth 21-23, YaneuraOu 23 s for depth 27-28
  (its nps ~4x ours at 16 threads). Our allocation (remaining/30 + byoyomi) spent 88 s per move in
  plies 3-40; YaneuraOu kept ~1000 s more clock throughout.
- Two decisive moves (judge):
  - Ply 37 3g4e: −196, best 6g6f −10 .. −35 (our score for 3g4e: +58).
  - Ply 45 5c4a+: −447 vs **4f4d −292**, the main thread's choice all through its search (depth
    22-23, score falling −40, −99, −129). A Lazy SMP helper replaced it: `LazyHelpers::finish`
    plays a helper move when the helper completed a deeper iteration with a higher score than the
    main thread's (`lazy_smp.rs:115`, `search_engine.rs:17169`); here the main score was a falling
    one. Helper moves replaced the main thread's on 4 of our 56 moves (plies 5, 9, 45, 49).

## 12. Run f2 games 1-4 and the Mac GUI game of 2026-11-20

Run f2 (PC, 0425008 code, 16 threads, 256 MB each, 60 min + 10 s, paired 2-ply random openings,
seed 50511; logs `matches/s45/full/f2.*`, analysis `matches/s45/full/judge/f2_g*`). Stopped
after game 4 (user, 2026-11-20): 0.5 of 4.

| Game | Opening (random plies) | Ours | Start (Suisho5, ours' view) | Result |
|---|---|---|---|---|
| 1 | 6g6f 9a9b | Black | about +150 | draw, repetition at 58 (both engines scored 0) |
| 2 | same | White | about −150 | loss, 149 |
| 3 | 2h4h 4a4b | Black | about −100 | loss, 144 |
| 4 | same | White | about +100 | lost at ply 120 (−2000) |

- Games 2 and 3: a slow slide from the starting deficit; ours' own eval followed the judge's
  (no large optimism), and no single move cost more than ~200 cp before the position was lost.
- Game 4, from the better side: level +100 to ply 42, then ply 44 6a5b −125 vs 9c9d +52 (ours
  scored it +222, main thread's choice, depth 22) and ply 56 3c3d −491 vs P*4g −337 (ours +42).
  From ply 45 to 60 the judge went −118 → −378 while ours stayed +31..+75.
- Mac GUI game (`matches/user_games/2026-11-20_mac_even_loss132.txt`, 3eec8b3 vs YaneuraOu
  halfkpvm): level to 64; ply 65 2i3g −157 vs 2e2d −2 (ours +12); ours above +25 while the judge
  went to −305 by 71. Three fresh replays of ply 65: 2e2d, 2e2d (by helper override), 2i3g.
- Speed in these games (last info line, moves over 5 s): ours 3.39M nps median, YaneuraOu 14.2M
  (4.2x); YaneuraOu's 256 MB hash 96% full (median hashfull 960). Ours reports no hashfull.
- Helper moves replaced the main thread's on 23 of our 230 f2 moves; the two decisive game 4
  moves were the main thread's.
- Time: ours `time/30 + byoyomi` (`src/usi.rs:280`) spends 80-130 s per opening move against
  YaneuraOu's 15-110 s; by ply 100 of game 3 ours had 9 min left, YaneuraOu 34 min; in game 4
  ours 32 min vs 38 at ply 44 and 9 vs 26 at ply 105 (an earlier note here had game 4's clocks
  the wrong way round; corrected after the council's Sol member checked the log).
- Pattern over the five losses analysed (f1, 11-19 Mac, 11-20 Mac, f2 g4; g2/g3 started behind):
  a middlegame move the main thread scores +50..+220 that Suisho5 scores 150-350 lower, then
  10-20 moves of over-optimism while the position goes.

## 13. Council (2026-11-20, direction: over-optimism at long time controls)

Members: Claude Opus 5.5 (high), GPT-6.2 Sol (high). Gemini failed twice (Gemini 4 Neon not on the
Cursor account; Gemini 4 Argon: monthly usage limit); not replaced. Brief:
`matches/s45/council_optimism_brief.md`.

- Opus: (1) long-TC time management (30 s per-iteration re-search cap `search_engine.rs:16514`
  fires on 64 of 199 long moves and drops fail-lows; no extension when the score falls; heavy
  opening spending), (2) the Lazy SMP selection rule `lazy_smp.rs:115`, (3) network
  (outcome weight 0.3). Belief to test: "the optimism is the network" — run YaneuraOu's own search
  with halfkpvm on the survey.
- Sol: (1) network coverage of quiet threats, (2) LMR on quiet replies, (3) time reserve released
  on instability. Corrected §12's game 4 clock claim (checked, right).
- Both: no games tonight; cheap replays/diagnostics first. Adopted.

Checks run (2026-11-20):
- YaneuraOu's search + halfkpvm, 1 thread, 2M nodes, the 414 survey positions
  (`matches/s45/survey/yoscale.py`, `yo_2m.jsonl`), bias vs Suisho5: **−18** where Suisho5 < −300
  (ours with the same network +219, ours with a1 +387); all +20 (ours +131 / +204); best move =
  Suisho5's 70% (ours 57% / 56%). Most of the optimism is our search, not the network class.
- Node ladder, 60 positions, 8M and 32M nodes (`ladder.jsonl`): behind (40), bias vs Suisho5 at 32M:
  ours a1 +480 (2M) → +427 (8M) → +402 (32M); YaneuraOu-halfkpvm +24 (8M), −88 (32M). Ours barely
  moves with 16x the nodes: a systematic score bias, not a horizon.
- Helper overrides (21 open positions of f1/f2, Suisho5 4 threads 10 s per candidate,
  `full/judge/override_*.jsonl`): helper move minus main move +47 mean (9 better, 4 worse, 8 within
  20 cp). The selection rule is not hurting on average: Opus's candidate 2 dropped.
- Per-rule ablation (`survey/ablate.py`, `ablate.jsonl`; 368 positions with Suisho5 < +100, ours a1,
  1 thread, 2M nodes; one switch per arm), bias where Suisho5 < −300 (n=158): base +387,
  correction history off +388, LMR off +415, null move off +383, rfp/razor/futility/LMP off +440,
  singular extension off +414. No single rule makes the optimism.
- Static (go depth 1) on the same 158: ours a1 +571, ours + halfkpvm +494, YaneuraOu + halfkpvm
  +597: our halfkpvm implementation is not the cause (YaneuraOu's static is the more optimistic).
  So from static to 2M nodes YaneuraOu's search removes ~615 cp of optimism, ours ~275 (halfkpvm)
  or ~185 (a1), and 16x the nodes adds only ~80 (ladder). Next: compare the PVs where the gap is
  largest, to see what kind of opponent resource our search does not find (drops, mating attacks,
  quiet threats) — a structural blind spot rather than a tuning margin.
- PVs (`survey/pvcmp.txt`, 2M nodes, the 20 largest halfkpvm-vs-YaneuraOu gaps): our score swings
  hundreds of cp between iterations (e.g. +405, +17, +326, +459, then −438 at depth 17 at 8M where
  YaneuraOu says −1353); some PVs are a single move with optimistic scores. Exact-zero scores in
  the 158 losing positions: base 1, hk 2, YaneuraOu 1, so repetition draws are not the cause.
- PC replay (`matches/s45/replay`, `replay.out`, `judge.jsonl`): 21 decisive positions (f1 plies
  37–51, f2 game 4 plies 38–56, Mac 11-20 plies 63–67), 16 threads, 256 MB, game move time T;
  arms base / no 30 s cap / no cap 2.5T / LMR off. 12 positions had differing moves; each was
  judged by Suisho5 (12 threads, 30 s MultiPV 4, then 15 s per candidate). Mean loss against the
  best candidate: base −54, no cap −41, no cap 2.5T −47, LMR off −43 (n=12, noise level). No arm
  is consistently better: no cap fixes f1 37 (−177 → −10) and Mac 65 (−236 → 0) but loses
  Mac 63 (104 → 5) and f2g4 56 (−328 → −427); 2.5T keeps the bad base move at f1 45 and Mac 65.
  At f2 game 4 plies 38–46 all arms, 2.5T included, still score +117..+377. Base itself picked
  9c9d (+74) at ply 44, not the 6a5b the game played: move choice there is run-to-run noise.
  Conclusion: neither the 30 s cap, time nor LMR is the lever; the optimism survives all of them.
  Time-allocation and cap changes are dropped as candidates for now.

### 13.1 Blind-spot trace (2026-11-20, `matches/s45/survey`)

- `tracepv.py` / `tracepv2.py`: down YaneuraOu's own PV in the 20 pvcmp positions (lines of 9-37
  plies), ours + halfkpvm and YaneuraOu + halfkpvm at 2M nodes, scored from the root side. Our
  search mostly plays YaneuraOu's moves (57/80 with the losing side to move, 44/64 with the
  winning side) but stays more optimistic everywhere along the line: ours − YaneuraOu +544 at
  the root, +671 / +527 / +428 at plies 4 / 8 / 12+, +270 at the PV's end. No single missed
  reply: the optimism is spread over the whole line. Our score also swings twice as much from
  one ply to the next (median 313 vs 141 cp). (The depth-1 columns of `tracepv2` are not
  static: YaneuraOu's reused its table from the 2M search just before. Use `ablate.jsonl`.)
- The one-move PVs in `pvcmp.txt` are a reporting artefact, not a search result: when the last
  iteration does not finish, the periodic info line repeats the previous depth's score under the
  new depth with only the move as PV.
- Mate in one at every node (`SHOGI_LEAN_MATE1=1`, `mate1.py`, a1, 2M nodes, the 368 ablation
  positions): bias where Suisho5 < −300 +393 vs base +387, same depth, same move agreement. Ruled out.
- **Equal nominal depth** (`eqdepth.py`, `eqdepth15.py`; same halfkpvm network, 1 thread, fresh
  engines, go depth N; bias vs Suisho5 on the 158 positions with Suisho5 < −300):

  | depth | ours | YaneuraOu | nodes ours / YaneuraOu (median) |
  |---|---|---|---|
  | 6 | +366 | +467 | 7k / 2k |
  | 9 | +271 | +298 | 86k / 18k |
  | 12 | +231 | +152 | 568k / 115k |
  | 15 | +154 | +59 | 2.78M / 451k |
  | 2M nodes | +219 (d≈14) | −21 (d≈18) | |

  Level positions (n=132): both +39..+134 at every depth. So at equal depth our search corrects
  the network's optimism about as fast as YaneuraOu's (d12→d15: −77 vs −93), trailing it by
  ~80-95 cp from depth 12. The equal-nodes gap of §13 (+219 vs −21) is mostly the cost of a
  depth: ours needs ~5-6x the nodes for the same nominal depth (S35 measured ~5x; the ratio has
  not closed since). Correction to §13's "the optimism is our search": it is our search's
  **node efficiency**, plus ~90 cp per depth, plus a1's extra ~170 cp over halfkpvm.
- Next: where our nodes per depth go (search_bench `qnodes` share, nodes by node type and by
  move kind at the 158 positions), against YaneuraOu's published design for ideas only.

### 13.2 Where the nodes go, and what the "optimism" is (2026-11-21, `matches/s45/survey`)

- **Nodes per depth = singular extension.** `search-diag` depth 12 on 40 of the 158 losing
  positions (`diag/`, a1, 1 thread): 559k nodes with singular extension, **154k without**
  (YaneuraOu + halfkpvm 115k), q-share 49% / 41%. Singular tests extend 48% and multicut 33%.
  The gap below `sing_beta` of the extended moves (instrumented build, not kept): ≤ 20 cp 10%,
  21-100 12%, 101-300 28%, 301-1000 34%, > 1000 17%: the table move really is far better in most
  extended nodes, so a larger margin cannot cut much (S42: margin 16 still extended 40%, and failed
  at 4 threads). Extensions are already capped per path at the iteration depth (`lean.rs:793-797`);
  check extensions off was −9 in S41. The equal-depth table of §13.1 therefore compares unequal
  trees: our depth 12 contains far more extended plies.
- **Ladder with halfkpvm** (`ladderhk.py`, `ladderhk_nose.py`; 60 ladder positions, reference
  Suisho5 at 32M, not 2M — Suisho5 2M is itself +120 off it on the 40 behind):

  | behind (40) | 2M | 8M | 32M |
  |---|---|---|---|
  | ours + halfkpvm | +315 | +244 | +253 (d20) |
  | same, singular extension off | | +294 (d20) | +270 (d22.5) |
  | ours + a1 | +480 | +427 | +402 |
  | YaneuraOu + halfkpvm | | +20 (d22) | −123 (d26) |

- **The "optimism" is mostly score scale, not an offset.** Linear fit of each search's score on
  Suisho5 32M (60 positions): intercepts +17..+56 for all; slopes ours + halfkpvm 0.61 / 0.74 /
  0.69 (2M/8M/32M), ours a1 0.35 / 0.49 (2M/32M), YaneuraOu + halfkpvm 1.03 / 1.23 (8M/32M),
  Suisho5 2M 0.80. And **a1's scores are 0.56x halfkpvm's** at depth 1 and 0.59x after 2M (368
  positions; mean |score| 181 vs 239 static, 202 vs 295 at 2M). Our search widens the scale with
  depth less than YaneuraOu's does, and a1 starts on a narrower scale. Consistent with this, in
  the 20 survey positions where we are winning (Suisho5 > +300) ours is *below* YaneuraOu's search
  (a1 −56, halfkpvm −14 vs YaneuraOu +68). A uniform scale does not change minimax move choice.
- **Move quality at equal nodes shows no gap** (`moveloss.py`): every distinct best move on the 60
  positions scored by Suisho5 alone at 32M nodes (`searchmoves`); mean loss against the best
  candidate: ours a1 33 / 38 / 41 (2M/8M/32M), ours + halfkpvm 73 / 36 / 42, YaneuraOu + halfkpvm
  — / 50 / 33, Suisho5 36 / 41 / 34 (its own 32M move loses 34: the judge's noise floor). Best
  move = Suisho5 32M's: a1 32M 36/60, YaneuraOu + halfkpvm 32M 34/60, Suisho5 2M 37/60. (§4's
  70% vs 57% used Suisho5 2M as reference, which shares YaneuraOu's search code.) Resolution is
  only ~±10-15 cp per move with n=60, 1 thread, fixed nodes.
- **Correction to §13 and to commit bf9d238's summary ("search, not network, is the cause"):** the
  cp bias measured the scale of the scores, not the quality of the moves. At equal nodes on 1
  thread our move choice is not measurably worse than YaneuraOu + halfkpvm here. What remains
  for the long-TC losses: nodes (YaneuraOu + Suisho5 searches 4.2x our nps in the f2 games, and
  one time doubling = +246 at 16 threads, §6), 16-thread scaling, Suisho5's network over
  halfkpvm/a1, and game-level noise (6 games). Next: 1 → 16 thread time-to-depth and nodes for
  ours vs YaneuraOu (offline), and the speed work.

### 13.3 Thread scaling (2026-11-21, `matches/s45/threads`)

6 level middlegame positions from the user's games (`jobs.txt`), go movetime 10 s, 256 MB, one
engine and thread count at a time. Mac (M4 Max, 12P+4E, ~1.5 cores of desktop load): ours =
`engines/usi-engine-stable` (3eec8b3); PC (7800X3D, WSL driver, Windows exes): ours = `yse-s45see`
build (3eec8b3 code, native), YaneuraOu V9 AVX512VNNI + Suisho5. `analyse.py`; completion of depth
d = last info line labelled d that is followed by a deeper one.

| median nps (M) | 1T | 2T | 4T | 8T | 16T | 16T / 1T |
|---|---|---|---|---|---|---|
| Mac ours | 0.53 | 0.99 | 1.81 | 3.61 | 5.81 | 11.0 |
| Mac YaneuraOu + Suisho5 | 1.20 | 2.41 | 4.85 | 9.37 | 14.38 | 12.0 |
| Mac YaneuraOu + halfkpvm | 1.17 | 2.33 | 4.68 | 8.99 | 13.97 | 11.9 |
| PC ours | 0.38 | 0.73 | 1.39 | 2.67 | 3.57 | 9.4 |
| PC YaneuraOu + Suisho5 | 1.56 | 3.12 | 5.84 | 11.22 | 14.91 | 9.6 |

- **Thread scaling is not the gap**: nps scales the same for both (11.0 vs 12.0 Mac, 9.4 vs 9.6 PC),
  and time-to-depth speedups are small and noisy for both (Lazy SMP adds little nominal depth:
  ours 1.2-1.8x, YaneuraOu 1.0-2.0x at 2-16 threads; 6 positions).
- **The gap is per-thread speed, and it is twice as large on the PC**: 1-thread ours / YaneuraOu
  2.3x on the Mac, **4.1x on the PC** (16 threads 4.2x, as in the f2 games). Ours is 0.72x as fast
  on the PC as on the M4 Max; YaneuraOu is 1.30x faster on the PC (its x86 build uses AVX-512 VNNI;
  our x86 NNUE kernels are AVX2, `nnue.rs:1865`). Windows vs Linux is not it: §8's Linux build ran
  339k (0425008), the Windows 3eec8b3 build 380k. CapturedPieces is two `Vec`s cloned per move
  (`types/board.rs:11`), an allocation per node on every platform; not visible in §8's top items.
- Levers, in order of measured share (§8, 0425008; re-profile 3eec8b3 first): legal move lists
  generated in full at the nodes that search moves (≈86 legal/node, `diag/`; whether scoring and
  sorting are also done in full is to be checked in the code) where 84% of cuts come on the first move — a staged move picker is the standard form; x86 NNUE kernels
  (AVX-512/VNNI with runtime dispatch, x86 only, NNUE 27% of PC time); the rest of SEE.

## 14. Speed round 4 (2026-11-21, worktree `s45x86`, uncommitted)

PC profile of 3eec8b3 (Linux build `~/yse-s45prof2`, samply, the 6 `threads/jobs.txt` positions,
1 thread 5 s each and 16 threads 3 s; `matches/s45/pcprof/s3eec_top1t.txt`, `s3eec_top16t.txt`;
1-thread 408k nps, 16 threads 3.83M; fingerprint: `go depth 12` 428,032 nodes on Mac and PC):
NNUE ~31% (layer 2 15.7%, `forward_from_activations` 6.3%, make 3.9%, refresh 2.2%), SEE ~19%,
move generation and legality ~19% (`MoveTestFilter::test` 5.3%), ordering ~5%, correction keys
+ hand hash 5.1%, malloc/free 1.2%.

- **Layer 2 on x86 in one call** (`nnue.rs`, `layer_2_blocked_pairs_avx2/avx512`): the a1 network's
  activations (4,096-16,383) need 64-bit totals, so layer 2 ran as 16 calls of `layer_2_fixed_i32`
  per evaluation (one per 32-row block, ~5 non-zero pairs each), each storing, reloading and
  widening its sums through memory. Now one call keeps the 64-bit sums in registers, two 32-bit
  sets per block (latency), AVX2 or AVX-512 BW, with VNNI `vpdpwssd` when present (runtime
  dispatch, `SHOGI_NO_AVX512=1` forces AVX2). Exact: test `layer_2_blocked_pair_kernels_match_i64`
  (200 cases per kernel, all three ran on the 7800X3D). PC, 6 positions x depth 13, 3 rounds
  interleaved: **+6.6% (VNNI), +5.3% (AVX2 only)**, same 24.7M nodes and moves in every round
  (a second 2-round run: +4.3%). aarch64 unchanged (same per-block code).
- **Table move first** (`lean.rs`, `MoveGenerator::legal_move_like`; `SHOGI_TT_FIRST=0` = old):
  the table move is built alone (that piece's moves or the drops onto its square, legality and
  `gives_check` as `generate_legal_moves`; test `legal_move_like_matches_full_list`, field-equal
  over 40 random games incl. evasions) and searched before the rest is generated. Not
  search-identical: the rest is ordered after the table move's search, with updated history.
  Off = identical to the stable engine (Mac: 8,240,896 nodes, same moves). On, depth 13 on the 6
  positions: Mac 15.1 s → 10.9 s, PC 39.1 → 28.3 s (vs off), but mostly from a smaller tree
  (nodes 8.24M → 6.11M Mac, 16.48M → 12.22M PC); nps only +3%. So it is an ordering change whose
  value needs games (fixed-node screen fits: nps nearly unchanged), not a speedup.

- **Ship check of the layer-2 kernel alone** (bf9d238 + `nnue.rs`; Mac `matches/s45/round4/`, PC
  `matches/s45/pc_check_x86.sh` → `C:\Users\fgant\s45x86check`): Mac unit tests 1170 passed, 0
  failed; Mac g1108 fingerprint (plies 30-69, 200k nodes) 0 of 40 differ from
  `engines/usi-engine-stable`. PC Windows: 1171 passed, 1 failed (`test_memory_breakdown`, RSS not
  read on Windows, as in round 3); layer-2 tests also pass in the generic x86-64 build; g1108
  fingerprint vs the 3eec8b3 build 0 of 40 differ for native VNNI, native forced AVX2
  (`SHOGI_NO_AVX512=1`) and generic x86-64. `speed_ab` (8 positions x 1M nodes, 4 rounds): native
  +3.0%, generic +3.6% (generic's node total differs: the search reads the clock). Speed across
  the three PC measurements: +3.0% to +6.6%.
