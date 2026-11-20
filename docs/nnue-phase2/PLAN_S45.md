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
