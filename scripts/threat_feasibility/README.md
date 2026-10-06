# Threat-input feasibility (Session 32)

Does information about attacks on each king predict our evaluation's error, out of sample?

1. Sample positions with YaneuraOu labels (`corpus_hc/all.jsonl`, split by game) plus the
   eval-gap set, as JSONL with `sfen`, `yo_black`, `split` (train/test/eg).
2. `python3 ours.py ENGINE REPO_DIR JOBS < sample.jsonl > DIR/scored.jsonl` -- our depth-1 score.
3. `python3 fit.py DIR` -- ridge regression of p(YaneuraOu) - p(ours) on `features.py`
   (per king: enemy attacks on the king zone, defenders, escapes, enemy hand, nearby enemy
   pieces; each also times side to move), evaluated on held-out games and the user games.

Pure Python (no numpy). Result: see docs/nnue-phase2/SESSION_LOG_032.md.
