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
