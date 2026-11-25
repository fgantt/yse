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

From epoch 3 on, strength is flat at about −260 while val loss keeps falling, and the export's rounding error grows
(|int − float| 10.8, 12.8, 13.4, 12.7, 15.6 units over epochs 1-5).

Epoch 1 is undertrained: pearson 0.80 vs Hao labels against Suisho5's 0.857 on the same positions, and it compresses large
evals (slope 0.31; p90 |eval| 1008 vs labels 3000). Export rounding: |int - float| ~10 units (a1's engine format: 0.8):
worth reducing later (quantization-aware training or larger hidden weights).
