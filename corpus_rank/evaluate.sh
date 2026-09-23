#!/bin/bash
# Evaluate a candidate network (Session 29).
#
# Usage: corpus_rank/evaluate.sh <candidate.bin> <tag>
#
# Three measurements, in increasing order of cost and trustworthiness:
#
#   1. Six-piece handicap ladder vs YaneuraOu at 50 nodes. Session 28 found this
#      is the only opponent setting whose result responds to our changes -- at
#      even material we score ~3/24 whether YaneuraOu searches 1 node or 50, so
#      those games carry no information. We hold the odds as Black, and ONLY the
#      Black games count: the handicap belongs to one colour while the harness
#      alternates, so the aggregate is meaningless.
#
#   2. Head-to-head against the deployed network at 1 s/move. The only
#      measurement that survived Session 28's scrutiny. ~2 h for 120 games.
#
# --random-plies 2, not the default 8: a random opening squanders a material
# advantage asymmetrically, because the side with more pieces has more to hang.
set -u
cd "$(dirname "$0")/.."

CAND="$1"
TAG="$2"
ENG="$PWD/target/release/usi-engine"
YANE=/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1
YDIR=$(dirname "$YANE")

# Six-piece handicap: White gives up both lances, both knights, rook and bishop.
SFEN6="2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"

echo "=== 1. six-piece ladder: $TAG vs YaneuraOu 50 nodes ==="
./target/release/usi_match \
  --engine-a "$ENG" --cwd-a "$PWD" --options-a "NNUEFile=$CAND" --go-a "nodes 20000" \
  --engine-b "$YANE" --cwd-b "$YDIR" --go-b "nodes 50" \
  --options-b "USI_OwnBook=false;BookFile=no_book;Threads=1" \
  --start-sfen "$SFEN6" --random-plies 2 --games 24 --seed 2901 \
  --output "matches/rank_${TAG}_6mai.csv" > "matches/rank_${TAG}_6mai.log" 2>&1
echo "--- our games as Black (the side holding the odds):"
awk -F, 'NR>1 && $2=="black"{print $3}' "matches/rank_${TAG}_6mai.csv" | sort | uniq -c

echo
echo "=== 2. head-to-head vs deployed, 1 s/move ==="
./target/release/usi_match \
  --engine-a "$ENG" --cwd-a "$PWD" --options-a "NNUEFile=$CAND" \
  --engine-b "$ENG" --cwd-b "$PWD" --options-b "NNUEFile=nnue/default.bin" \
  --byoyomi 1000 --games 120 --seed 2902 \
  --output "matches/rank_${TAG}_h2h.csv" > "matches/rank_${TAG}_h2h.log" 2>&1
tail -3 "matches/rank_${TAG}_h2h.log"
