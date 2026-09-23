#!/bin/bash
# Sibling-ranking corpus generation (Session 29).
#
# Two sources:
#   - corpus_s28/all.jsonl: positions our own engine actually reaches. This is
#     where our mis-rankings cost us games.
#   - nnue_corpus_yaneura_d10.jsonl: teacher self-play, for coverage of strong
#     play. game_id % 20 == 0 is EXCLUDED -- that is the neutral held-out test
#     set from Session 28 and training on it would void the only move-loss
#     measurement we trust.
set -u
cd "$(dirname "$0")/.."
ENG=/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1
BIN=./target/release/rank-gen

for i in $(seq 0 7); do
  $BIN --engine-path "$ENG" --input corpus_s28/all.jsonl \
       --output corpus_rank/own$i.jsonl \
       --shard $i --shards 8 --stride 14 --depth 8 --multipv 8 \
       > corpus_rank/own$i.log 2>&1 &
done

for i in $(seq 0 1); do
  $BIN --engine-path "$ENG" --input nnue_corpus_yaneura_d10.jsonl \
       --output corpus_rank/teacher$i.jsonl \
       --shard $i --shards 2 --stride 1 --depth 8 --multipv 8 \
       --exclude-gameid-mod 20 \
       > corpus_rank/teacher$i.log 2>&1 &
done

wait
cat corpus_rank/own*.jsonl corpus_rank/teacher*.jsonl > corpus_rank/all.jsonl
echo "=== combined: $(wc -l < corpus_rank/all.jsonl) groups ==="
