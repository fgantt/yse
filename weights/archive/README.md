# NNUE weights archive

Untracked NNUE weight checkpoints from Phase 2 sessions, moved here in Session 19
to retire the 41-file sprawl at the repo root. None of these files are
referenced by tests or scripts in the source tree — `nnue_weights_trained.json`
remains at the repo root because `src/lib.rs` auto-loads it on engine
initialisation and `test_eval_paths_agree_on_trained_weights` looks for it.

The naming pattern roughly follows the session that produced each file:
- `nnue_weights_iter_*.json` — Session 1–4 PST-imitation checkpoints
- `nnue_weights_s9_*` — Session 9 relaxed-target experiments
- `nnue_weights_s10_*` — Session 10 symmetrise-output-layer experiments
- `nnue_weights_shadow_*` — Session 8/12 f32 shadow weight runs
- `nnue_weights_yaneura_trained_*` — Session 7 YaneuraOu corpus training
- `nnue_weights_*.bin` — Session 18+ bincode emissions

These files predate the binary-format change and are mostly JSON; loading any
through `NNUEWeights::load` works (extension dispatch handles `.json` and
`.bin` transparently). Future trainer outputs to the repo root are
gitignored — see `.gitignore` rule added in Session 19.
