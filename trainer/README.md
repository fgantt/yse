# GPU trainer (Session 38)

PyTorch trainer for the engine's NNUE (256x2 -> 32 -> 1, pov + hand + 9 king buckets, dual).
Runs on Apple MPS (~320k positions/s on an M4 Max), CUDA or CPU. It trains in float and rounds
onto the engine's integers once, at export; the old Rust trainer (`nnue-offline-trainer`) kept
the input layer on the integer grid every step, which stopped small updates.

## Setup

```bash
python3 -m venv trainer/.venv
trainer/.venv/bin/pip install torch numpy
```

Tested with torch 2.15, numpy 2.5.

## Data

`nnue_pack` (src/bin/nnue_pack.rs) turns a labelled JSONL corpus into 100-byte records, holding
out every 20th game for validation, and writes the feature lookup tables from the engine's own
code, so the trainer and the engine cannot disagree about what a feature is:

```bash
cargo run --release --bin nnue_pack -- pack --corpus corpus_s37c/pool_d8.jsonl --out-dir corpus_s38/packed_d8
cargo run --release --bin nnue_pack -- tables --king-buckets 9 --out trainer/tables_k9.json
```

## Train, export, check

```bash
cd trainer
.venv/bin/python train.py --data ../corpus_s38/packed_d8 --tables tables_k9.json \
    --out runs/x --epochs 8 --lr 1e-3 --gamma 0.7
../target/release/nnue-convert-weights --input runs/x/epoch_005.json --output x.bin
```

Each epoch writes engine JSON (`epoch_NNN.json`). Networks whose layer shifts are not 6 are
saved as v5 `.bin` files (`SHOGINNUE5`), which engines before Session 38 cannot load.
**Choose epochs by held-out loss, not pearson**: in Session 38 the epoch with the best pearson
lost to t8 and the one with the lowest loss won.

`verify.py` checks the trainer against the engine on `nnue_pack check` output (features,
integer emulation, float model):

```bash
cargo run --release --bin nnue_pack -- check --corpus <jsonl> --n 2000 --weights x.bin > check.jsonl
.venv/bin/python verify.py --check check.jsonl --tables tables_k9.json --weights runs/x/epoch_005.json
```

`--mid 32` adds a middle layer (`256x2 -> 32 -> 32 -> 1`, v6 files); in Session 38 it did
not beat the network without one.

`--init net.json` starts from an engine network (convert a `.bin` to `.json` first) instead of
from scratch; `--resume` continues from `runs/x/last.pt`.
