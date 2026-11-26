# PLAN S48: our first standard-format network (2026-11-24)

Step 1 of `docs/ROADMAP_DUAL_ENGINE.md`: a HalfKP 256x2-32-32 `nn.bin` from our trainer, run in YaneuraOu and in our engine.
Branch `s48-net` (035e516 on 17a2e37).

## Tools (035e516)

- `src/evaluation/yo_nnue.rs` loads plain HalfKP and HalfKP_vm (told apart by size); `halfkp_*` helpers give the layout.
- `nnue_pack tables --halfkp` (needs `--features yo-diagnostic`) writes `trainer/tables_halfkp.json` (125,388 rows,
  stride 1,548, no PSQT).
- `trainer/train.py --arch halfkp --header-from <a halfkp nn.bin>`: clipped activations, i8-range hidden weights, export
  `epoch_NNN.bin`; `--data a,b,c` draws batches from several folders in proportion to size.

## Checks

- YaneuraOu (the Suisho5 install, `EvalDir` pointed at our folder; its own files untouched) loads our file and searches
  normally (smoke net, depth 16 at 200k nodes, same move as Suisho5).
- Our loader's plain-HalfKP layout is right: our engine with Suisho5's file vs with halfkpvm (validated earlier), 10k
  nodes, 120 games: 55.0% (66-0-54).
- Scale: on 20,000 Hao held-out positions Suisho5's evals match the Hao labels' scale (|eval| median 576 vs 556, slope
  0.83, pearson 0.857), so Hao labels with sigmoid 600 are the right target. The Suisho5 depth-9 set's wider spread
  (median 729) comes from its positions, not its scale: it can join the data.

## Run h1 (PC GPU, started 16:15, `~/s48net/trainer/runs/h1`)

Hao s50q + s150q + s200bq (1.83B), 8 epochs x 300M, lr 1e-3, gamma 0.7, sigmoid 600, clamp 3000, outcome weight 0.3.
183k positions/s: ~27 min an epoch, done ~20:00. Per-epoch check (`matches/s48/netmatch/per_epoch.sh` -> `epochs.txt`):
YaneuraOu + ours vs YaneuraOu + Suisho5 at 10k and 100k nodes, 120 games each.

| Epoch | val pearson | val loss | 10k vs Suisho5 | 100k vs Suisho5 |
|---|---|---|---|---|
| 1 | 0.865 | 0.0372 | 1.7% (2-0-118); in our engine 7.9% | |
| 2 | 0.882 | 0.0347 | 9.2% (−398 ± 55) | 15.8% (−290 ± 43) |
| 3 | 0.891 | 0.0332 | 19.2% (−250 ± 40) | 18.8% (−255 ± 41) |
| 4 | 0.897 | 0.0324 | 15.0% (−301 ± 44) | 20.0% (−241 ± 40) |
| 5 | 0.900 | 0.0320 | 16.7% (−280 ± 43) | 18.3% (−260 ± 41) |
| 6 | 0.903 | 0.0316 | 21.7% (−223 ± 38) | (stopped) |

From epoch 3 on, strength is flat at about −260 while val loss keeps falling, and the export's rounding error grows
(|int − float| 10.8, 12.8, 13.4, 12.7, 15.6 units over epochs 1-5).

Epoch 1 is undertrained: pearson 0.80 vs Hao labels against Suisho5's 0.857 on the same positions, and it compresses large
evals (slope 0.31; p90 |eval| 1008 vs labels 3000). Export rounding: |int - float| ~10 units (a1's engine format: 0.8):
worth reducing later (quantization-aware training or larger hidden weights).

h1 was stopped after epoch 6 (18:59) for run h2: strength had been flat since epoch 3.

### Why h1 is weak: export rounding is not it, data is the likely cause

- **Rounding, by layer** (`trainer/quant_probe.py`, epoch 5, 12,000 val positions): mean |int − float| ft 4.7, w1 13.0,
  t1 1.7, w2 1.8, t2 1.0, out 7.5, all 15.1 units (matches `int_eval` to 0.1). w1's integers are tiny (median |w| 1,
  20% zero) and out's are 13. But the exported net tracks the labels exactly as well as the float one (pearson to
  teacher 0.9034 vs 0.9030): rounding adds noise, not error against the labels. a1's own format has i16 hidden weights at
  free scales, which is why a1 rounds to 0.8.
- **Fit to labels is not strength:** h1 epoch 5 tracks the Hao labels better (pearson 0.903) than Suisho5 does (0.857 on
  Hao held-out positions) and still loses ~−260 to it.
- **Against a1, in our engine** (yo-diagnostic build, 1 thread, fixed nodes, 120 games): h1 epoch 5 vs a1 at 10k 23.8%
  (28-1-91, about −200), at 100k 20.0% (23-2-95, about −240).
- **Data:** h1 trained on Hao only. In S39, Hao alone lost 90-150 to our own data in a1's format, and only the mix
  (ours + Hao) shipped (m8, then a1 = ours_quiet + s50q + s150q). h1 left our data out.

### Run h2 (PC GPU, started 18:59, `~/s48net/trainer/runs/h2`, `matches/s48/s48_pc_h2.sh`)

ours_quiet listed twice (~14% of draws; a1 19.6%, mix_h2 10.8%) + Hao s50q + s150q + s200bq + Suisho5 `packed_s5`
(3.19B positions). packed_s5 has no held-out file: its val is the last 2M records of its train.bin, so ~1/6 of val is
not held out. 16 epochs x 300M, lr 1e-3, gamma 0.84, sigmoid 600, clamp 3000; ~27 min an epoch, done ~02:20.
Per-epoch check (`matches/s48/netmatch/per_epoch_h2.sh` -> `epochs_h2.txt`, even epochs): our engine with it vs with
a1 at 10k (the replace-a1 target), and YaneuraOu + it vs YaneuraOu + Suisho5 at 100k.

Replacing a1 needs, beyond the fixed-node rows: a first-class standard-format loader in our engine (incremental, SIMD
with runtime dispatch; the diagnostic loader recomputes from scratch), then the gate (pooled SPRT [0, 40], 1 s, 4 threads).

h1 epoch 6 at 100k vs Suisho5 (finished after the stop): 22.9% (26-3-91, −211 ± 38).

### Standard-format loader in the normal build (f9b1d68)

`src/evaluation/nnue_std.rs`: `NNUEFile` may name an `nn.bin` (HalfKP or HalfKP_vm 256x2-32-32). On load it is converted
into our own king-conditioned POV layout (81 buckets, hand thermometer, dual, one middle layer), so the incremental
accumulator, make/unmake stacks and SIMD layer 2 run unchanged; `NNUEWeights::clipped` switches to the format's
arithmetic (clip [0, 127], bias before shift, output / 16, so scores are in the format's units, ~1.7x a1's).

- Exact: `std_nnue_check` (yo-diagnostic) vs the from-scratch diagnostic on h1 epoch 6 and Suisho5's file: 4,000 static
  scores and ~15,000 incremental search nodes each, 0 mismatches.
- Our own networks unchanged: `search_bench` fingerprint on a1 identical to main (30 positions, depth 7).
- Speed (Mac, 1 thread, 4 positions x 4 s, matches running alongside): a1 627k nps, h1 583k, Suisho5 605k; the
  diagnostic path 470k.

Open for replacing a1: score scale (our search's margins were tuned on a1's narrower scale; a scale option, checked at
fixed nodes, is cheap), then the gate at 1 s, 4 threads.

### h2 results (done 02:06, 16 epochs)

Our engine with h2 vs with a1 (fixed 10k nodes, 1 thread, 120 games) and YaneuraOu + h2 vs YaneuraOu + Suisho5 (100k):

| Epoch | val pearson | val loss | vs a1, our engine, 10k | vs Suisho5, YaneuraOu, 100k |
|---|---|---|---|---|
| 2 | 0.896 | 0.0334 | 17.5% (21-0-99) | 16.2% (−285 ± 43) |
| 4 | 0.916 | 0.0304 | 27.5% (33-0-87) | 24.2% (−199 ± 37) |
| 6 | 0.924 | 0.0294 | 33.3% (40-0-80) | 23.8% (−203 ± 37) |
| 8 | 0.930 | 0.0288 | 40.8% (49-0-71) | 37.1% (−92 ± 33) |
| 10 | 0.932 | 0.0284 | 39.6% (47-1-72) | 30.4% (−144 ± 34) |
| 12 | 0.934 | 0.0281 | 41.7% (50-0-70) | 30.8% (−140 ± 34) |
| 14 | 0.934 | 0.0280 | 41.7% (50-0-70) | 39.6% (−73 ± 32) |
| 16 | 0.935 | 0.0279 | 44.2% (53-0-67, about −40 ± 60) | 36.2% (−98 ± 33) |

The data mix was the fix: h2 is ~120-150 stronger than h1 against Suisho5 and still climbing slowly against a1 at the
last epoch (val loss still falling, lr 6e-5). Not yet equal to a1. The a1 rows play the standard net at its own score
scale (~1.7x a1's); the scale option is not tested yet.

### Score scale (508aecd, `NNUEScale`)

h2 epoch 16 in our engine (normal build, `NNUEFile`) vs a1, 10k nodes, 120 games: scale 100 44.2% (53-0-67, the same
games as the diagnostic build), 80 44.6% (53-1-66), 60 39.2% (47-0-73). The scale is not the gap; keep 100.

### Run h3 (PC GPU, started 04:23, `~/s48net/trainer/runs/h3`, `matches/s48/s48_pc_h3.sh`)

Warm start from h2's last.pt (`train.py --warm`: weights and data positions, fresh Adam), lr 2e-4 gamma 0.78, 10 epochs
x 300M, same data except Suisho5 is `packed_s5` with its real held-out val.bin (9.2M; h2's s5v val was the tail of
train). h2 epoch 16 on this val: pearson 0.9348, loss 0.02808. Per-epoch check `per_epoch_h3.sh` -> `epochs_h3.txt`.

Toward Suisho5: the full Suisho5 depth-9 set (~5B positions; S40 packed only the first 922.7M) is downloading to the PC's
WSL disk (`~/data/s5_7z`, `matches/s48/s5_fetch_all.sh`) for an h4 run on ~7B positions.

h3 epoch 2: val pearson 0.9356, loss 0.02810 (h2 0.02808 on the same val); vs a1 46.7% (55-2-63); vs Suisho5 36.2%
(−98 ± 33).

### Field prior for the Suisho5 bar (nodchip's tanuki- blog, read 2026-11-25; ideas only)

- 2022-06-07: nnue-pytorch, halfkp_256x2-32-32 (our format) on 8 x 10^9 positions generated by Suisho5 (depth 9, 50k
  nodes, floodgate 3900+ starts), batch 16384: **−56 ± 10 vs Suisho5's own 256x2-32-32 net** (5000 games, 300 s + 2 s).
  So an experienced trainer with ~8B Suisho5-generated positions still fell short of Suisho5's net; h2 is −73 to −98.
- 2022-10-31: plain SGD (lr 1.0) with a reduce-on-plateau schedule (halve when loss stops improving, 50 epochs of 10M)
  beat Ranger by +33 ± 15 on the same data; still −47 vs YaneuraOu's official net.
- Their settings: lambda 0.5 (ours 0.7 = outcome weight 0.3), win-rate scaling 361 (ours sigmoid 600: their constant
  is in their own units, not checked against ours), no quantization-aware training (quantize after convergence),
  per-epoch weight clipping; momentum SGD +30 ± 10 over plain SGD (2024-01-16, 1024x2-8-32).
- 2023-10-22: Hao-generated vs Suisho5-generated data: equal (+0.9 ± 9.6) though Hao's loss is higher.

Reading: data volume is necessary but probably not sufficient for parity; the optimizer/schedule and lambda are the
next levers worth single-variable tests, each checked in YaneuraOu vs Suisho5 at fixed nodes.
h3 epoch 4: val pearson 0.9357, loss 0.02799; vs a1 43.3% (52-0-68); vs Suisho5 34.6% (−111 ± 33). Flat against h2
epoch 16: more epochs on the same data have stopped paying.
h3 epoch 5: val loss 0.02790; stopped there (flat) for the h4 pilots.

### h4 pilots (user chose pilots first, 2026-11-25; `matches/s48/s48_pc_pilots.sh`, Mac side `netmatch/pilots.sh`)

One change each from h2's recipe, 6 epochs x 300M, same data and seed, so h2 epoch 6 is the baseline:
p1 = outcome weight 0.5 (lambda 0.5); p2 = SGD momentum 0.9 (`train.py --optimizer sgd`) at the best lr of a 30M-position
sweep (0.3 to 30). Each epoch-6 net: 360 games vs Suisho5 in YaneuraOu at 100k (seeds 58000 x 10 + 58100 x 20, ±~20
Elo) and 240 vs a1 in our engine at 10k. `--plateau N` (halve lr after N epochs without val improvement) is ready
for the long h4 run.

| Pilot (epoch 6) | vs Suisho5, YaneuraOu, 100k (360 games) | vs a1, our engine, 10k (240 games) |
|---|---|---|
| h2 (baseline: Adam, outcome weight 0.3) | 107-6-247, 30.6% (about −142) | 77-1-162, 32.3% |
| p1 (outcome weight 0.5) | 70-4-286, 20.0% (about −241) | 72-0-168, 30.0% |

p1 is ~100 worse against Suisho5: rejected; keep outcome weight 0.3. (Its val loss is on a different target, not
comparable.)

SGD lr sweep (1 epoch x 30M from scratch; Adam 1e-3: val loss 0.0469, pearson 0.833): lr 0.3 0.0535 / 0.806, 1 0.0527
/ 0.806, 3 collapsed (pearson 0.000), 10 0.0598 / 0.796, 30 collapsed. p2 uses lr 1; unstable above it.

Full Suisho5 set: 7,070,161,203 records after packed_s5's 922.7M (8.0B in all). Packed at 100 bytes a record, all of it
would need 707 GB and C: has ~600 GB free, so 4 parts of 1B (`packed_s5_p1`-`p4`) are being packed; the raw chunks for
the other ~3B stay in `suisho5/raw2`.
| p2 (SGD momentum 0.9, lr 1) | 63-4-293, 18.1% (about −263) | 57-1-182, 24.0% |

p2 is ~120 worse against Suisho5 at equal positions: rejected for now (SGD learns slower; a long plateau-scheduled run
might differ, untested). Both pilots lost, so h4 is h2's recipe with more data and a longer schedule.

Packing: the Windows packer took the out dir `/mnt/c/...` as `C:\mnt\c\...` (part 1 landed there; moved, script now
passes a relative dir).

### Run h4 (PC GPU, starts when `packed_s5_p4` is done, ~13:40; `matches/s48/s48_pc_h4.sh`)

h2's recipe on ~7.1B positions: ours_quiet x3 (~9% of draws), Hao s50q + s150q + s200bq, Suisho5 packed_s5 + p1-p4. From
scratch, 48 epochs x 300M (14.4B draws), lr 1e-3 gamma 0.93; ~21 h. Per-epoch check every 4th epoch
(`netmatch/per_epoch_h4.sh` -> `epochs_h4.txt`): vs a1 in our engine at 10k, vs Suisho5 in YaneuraOu at 100k, 120 each.
Expectation from the field prior (nodchip, 8B Suisho5-generated positions, same format): around −50 vs Suisho5.

h4 so far (h2 at the same epoch in brackets; 120 games each):

| Epoch | val pearson / loss | vs a1, 10k | vs Suisho5, 100k |
|---|---|---|---|
| 4 | 0.918 / 0.0307 | 22.5% (27.5%) | 18.3%, −260 ± 41 (24.2%, −199) |
| 8 | 0.935 / 0.0288 | 36.2% (40.8%) | 22.1%, −219 ± 38 (37.1%, −92) |
| 12 | 0.939 / 0.0282 | 38.3% (41.7%) | 25.0%, −191 ± 37 (30.8%, −140) |
| 16 | 0.941 / 0.0279 | 39.2% (44.2%) | 32.5%, −127 ± 34 (36.2%, −98) |
| 20 | 0.943 / 0.0276 | 40.4% | 38.3%, −83 ± 33 |
| 24 | 0.944 / 0.0274 | 54.2% (65-0-55) | 40.4%, −67 ± 32 |
| 28 | 0.945 / 0.0272 | 54.2% (65-0-55) | 35.8%, −101 ± 33 |
| 32 | 0.945 / 0.0271 | 47.5% (57-0-63) | 41.2%, −61 ± 32 (export |int − float| 24.3, max 231) |
| 36 | 0.945 / 0.0270 | 47.5% (57-0-63) | 40.4%, −67 ± 32 |
| 40 | 0.946 / 0.0270 | 45.0% (54-0-66) | 42.9%, −50 ± 32 (export |int − float| 23.5, max 145) |

Behind h2 at equal epochs, by about the noise; h4's lr is higher (0.93/epoch vs 0.84). Val loss is on a different val
mix (more Suisho5 folders), so not comparable with h2's. Decision point: epoch 16; if still behind h2, rebalance the mix.
Epoch 16: within noise of h2's last epoch (−30 ± 50 against Suisho5) and still rising (lr 3.1e-4, h2 ended at 6e-5):
continue to epoch 48.

Ceiling check queued after epoch 48 (`netmatch/ceiling.sh` -> `ceiling.txt`): in our engine, 1 thread, Suisho5's file vs
a1 and h4 e48 vs a1 at 10k and 100k (360 games each) and 1M (120). Reason: in our engine a1 ≈ halfkpvm (+3 ± 33,
PLAN_S44) and Suisho5's file vs halfkpvm was only 55.0% at 10k, so a network alone may top out far below the user's
"win 8 of 10 vs a1" (+240); the check says how much a network can give and whether search should come first.
