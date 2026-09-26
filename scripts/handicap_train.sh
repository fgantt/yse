#!/bin/bash
# Session 30: fine-tune the deployed network with handicap positions.
#
# Three arms, identical except the corpus, so the handicap data's effect is
# separated from ordinary training drift (Session 29: training on the value
# corpus alone at lr 0.005 already made the network worse, hence lr 0.001):
#   control -- the existing value corpus only
#   x1      -- plus corpus_hc/all.jsonl once
#   x4      -- plus it four times
# Judge by per-epoch checkpoints, on the corner-promotion test first, then by
# head-to-head against the stable engine.
set -u
cd "$(dirname "$0")/.."
TRAINER=${TRAINER:-./target/release/nnue-offline-trainer}
OUT=weights/session30
mkdir -p "$OUT"
common=(
  --init-weights nnue/default.bin
  --epochs 2 --checkpoint-every 1
  --batch-size 256 --learning-rate 0.001
  --outcome-weight 0.3 --eval-clamp 2000 --target-eval-scale 600 --sigmoid-eval-scale 410
  --seed 47 --use-adam --use-hand-features --use-pov-features --king-features
  --dual-perspective --use-sigmoid-loss --king-factorizer --king-buckets 9
  --validate-sample 5000
)
"$TRAINER" "${common[@]}" --corpus corpus_rank/value_sub.jsonl --output-weights "$OUT/control.bin" > "$OUT/control.log" 2>&1 &
"$TRAINER" "${common[@]}" --corpus corpus_hc/mix_x1.jsonl --output-weights "$OUT/x1.bin" > "$OUT/x1.log" 2>&1 &
"$TRAINER" "${common[@]}" --corpus corpus_hc/mix_x4.jsonl --output-weights "$OUT/x4.bin" > "$OUT/x4.log" 2>&1 &
wait
for a in control x1 x4; do echo "=== $a"; grep -E "^Epoch" "$OUT/$a.log"; done
