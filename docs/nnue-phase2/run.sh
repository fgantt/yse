#!/bin/zsh
# 100K self-play games, played by the Session 27 engine and labelled by
# YaneuraOu at depth 10.
#
# Why this size: the Session 26 dose curve showed 12_000 games support only
# 3-6 passes before the network memorises them (+65 Elo at 3 passes, -7 at 15).
# Game count, not epochs, was the limit on that gain.
#
# The player is engines/usi-engine-s27: today's build, ~+145 Elo and ~30%
# faster than the one that generated corpus_self, so the positions come from
# better play. Game ids start at 1_000_000 to stay clear of corpus_self
# (500_000-580_000).
ENG=/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1
PLAYER=$PWD/engines/usi-engine-s27
probs=(0 0 0 0 0.05 0.05 0.05 0.05)
for i in {1..8}; do
  ./target/release/corpus-gen --engine-path $ENG --player-engine $PLAYER --player-go "nodes 3000" \
    --games 12500 --depth 10 --hash-mb 64 --random-move-prob ${probs[$i]} \
    --seed $((2800 + i)) --game-id-offset $((1000000 + i * 20000)) \
    --output corpus_s28/part$i.jsonl > corpus_s28/part$i.log 2>&1 &
done
wait
