# Session 34 analysis scripts

Run from the repo root. Engines are launched with cwd = repo root (so `nnue/default.bin` and
`resources/magic_tables` are found) and `SHOGI_PREFS_DIR=matches/s34_threads/prefs`, so they
never touch the GUI's thread setting. Paths to YaneuraOu are hard-coded.

| Script | Use |
|---|---|
| `analyze_loss.py GAME b NODES` | Per-move: our score/move (1 s) vs YaneuraOu (best, score of our move). GAME is a `matches/user_games/*.txt` file |
| `probe_pos.py GAME PLY yane_nodes,.. ours_ms,..` | One position: YaneuraOu at node budgets, ours at times |
| `tt_time.py ENGINE MS STEP MOVESFILE opt=val..` | Fixed time per move over a game, table kept: mean depth. **The main offline check for anything stateful or multi-threaded** (16 threads, 10000 ms) |
| `ttd_seq.py ENGINE DEPTH STEP MOVESFILE opt=val..` | Time to a fixed depth over a game, table kept |
| `repeat.py ENGINE SFENFILE "go .." N opt=val..` | Repeat one search; flags hangs (reader thread; never mix `select` with buffered reads) |
| `odds_compare.py OUT W NW csv..` | Our move vs YaneuraOu's on the odds side of YaneuraOu-vs-YaneuraOu games, judged at 3M nodes |
| `odds_rejudge.py IN OUT W NW` | Re-judge with uncapped scores (mates = +-(100000 - plies)) |
| `odds_classify.py compare.jsonl` | Move-kind tallies (drops into enemy camp, rook/bishop moves, ...) and disagreements |
| `sample_callers.py SAMPLE.txt REGEX [levels]` | Callers of a function in a macOS `sample` profile |
