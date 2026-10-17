#!/bin/bash
# Handicap corpus (Session 30): positions our engine reaches in handicap games,
# labelled by YaneuraOu.
#
# Why: the teacher self-play corpus never has one side thousands of centipawns
# ahead, and the network misjudges such positions. In a won 8-piece handicap
# game it scored every corner promotion 100-430 cp above YaneuraOu's preferred
# move (YaneuraOu rated them equal), so the engine collected promotions in a
# corner for 60 moves instead of attacking.
#
# Our stable engine plays both sides (so the corpus contains the positions it
# actually steers into, including the aimless ones); the teacher labels every
# position. Starts rotate through 2-, 4-, 6- and 8-piece odds with the handicap
# giver (White) to move, as in a real handicap game. --random-plies 2: more
# randomness squanders material asymmetrically (Session 28).
#
# Usage: scripts/handicap_corpus.sh [games_per_worker] [workers]
set -u
cd "$(dirname "$0")/.."
GAMES=${1:-150}
WORKERS=${2:-8}
OUT=corpus_hc
mkdir -p "$OUT"
YANE=${YSE_YANE:-$(python3 scripts/paths.py yane)} || exit 1
GEN=${CORPUS_GEN:-./target/release/corpus-gen}
PLAYER=$PWD/engines/usi-engine-stable

H2="lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
H4="1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
H6="2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
H8="3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"

for i in $(seq 0 $((WORKERS - 1))); do
  "$GEN" --engine-path "$YANE" --games "$GAMES" --depth 8 --random-plies 2 \
    --player-engine "$PLAYER" --player-cwd "$PWD" --player-go "nodes 3000" \
    --start-sfen "$H2" --start-sfen "$H4" --start-sfen "$H6" --start-sfen "$H8" \
    --seed $((3000 + i)) --game-id-offset $((i * 100000)) \
    --output "$OUT/part$i.jsonl" > "$OUT/part$i.log" 2>&1 &
done
wait
cat "$OUT"/part*.jsonl > "$OUT/all.jsonl"
echo "=== $(wc -l < "$OUT/all.jsonl") positions from $((GAMES * WORKERS)) games ==="
