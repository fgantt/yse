#!/bin/bash
# Download the files listed in sample50.txt (path size sha256) from nodchip/shogi_hao_depth9
# (MIT) into raw/, verifying size and sha256; skips files already verified.
cd "$(dirname "$0")"  # run from a copy in corpus_hao/, next to sample50.txt; files go to raw/
mkdir -p raw
U=https://huggingface.co/datasets/nodchip/shogi_hao_depth9/resolve/main
while read f size sha; do
  [ -f raw/$f.ok ] && continue
  curl -sfL --retry 5 -o raw/$f.part "$U/$f" || { echo "FAIL $f"; continue; }
  got=$(shasum -a 256 raw/$f.part | cut -d' ' -f1)
  if [ "$got" = "$sha" ] && [ $(stat -f %z raw/$f.part) = $size ]; then
    mv raw/$f.part raw/$f; touch raw/$f.ok; echo "ok $f $(date +%T)"
  else echo "BAD $f"; rm -f raw/$f.part; fi
done < sample50.txt
echo "fetch done $(date)"
