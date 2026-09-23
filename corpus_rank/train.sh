#!/bin/bash
# Session 29 scale-up: does the pairwise ranking hinge survive at full size?
#
# Two arms, identical in every respect except the ranking term, so any
# difference is attributable to the objective rather than to more training:
#
#   control  -- value regression only (--rank-weight 0). Still *measures*
#               held-out ranking, which is what makes it a control rather than
#               just a baseline.
#   pairwise -- the same, plus the hinge at weight 1.
#
# Checkpoint every epoch and judge by the checkpoints, NOT by the trainer's
# "best" file: that is selected on held-out Pearson r, which Session 28 showed
# rises monotonically while Elo moves the other way.
#
# At small scale the ranking gain appeared in epoch 1 and eroded after, so the
# early checkpoints matter most.
set -u
cd "$(dirname "$0")/.."
mkdir -p weights/session31

VALUE=corpus_rank/value_sub.jsonl
RANK=corpus_rank/all.jsonl

common=(
  --corpus "$VALUE"
  --rank-corpus "$RANK"
  --init-weights nnue/default.bin
  --epochs 4
  --checkpoint-every 1
  --batch-size 256
  --learning-rate 0.005
  --outcome-weight 0.3
  --eval-clamp 2000
  --target-eval-scale 600
  --sigmoid-eval-scale 410
  --seed 47
  --use-adam
  --use-hand-features
  --use-pov-features
  --king-features
  --dual-perspective
  --use-sigmoid-loss
  --king-factorizer
  --king-buckets 9
  --validate-sample 5000
  --rank-temperature 100
  --rank-groups-per-batch 32
)

./target/release/nnue-offline-trainer "${common[@]}" \
  --rank-weight 0 \
  --output-weights weights/session31/control.bin \
  > weights/session31/control.log 2>&1 &

./target/release/nnue-offline-trainer "${common[@]}" \
  --rank-weight 1 --rank-pairwise \
  --output-weights weights/session31/pairwise.bin \
  > weights/session31/pairwise.log 2>&1 &

wait
echo "=== control ==="
grep -E "^Epoch" weights/session31/control.log
echo "=== pairwise ==="
grep -E "^Epoch" weights/session31/pairwise.log
