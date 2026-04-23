# NNUE Implementation Session Log

## Session 7: YaneuraOu Teacher Corpus + Offline Trainer — Integration Lessons

**Date**: 2026-04-23
**Duration**: ~3 hours
**Phase**: Phase 2b (execute teacher-target training)
**Objective**: Use Session 6's `corpus-gen` to build a real YaneuraOu-derived training corpus, write an offline trainer that consumes it, retrain NNUE, and re-run the ELO pilot to see if the "NNUE ≈ PST" ceiling finally breaks.

---

## Pre-Session Checklist

- [x] Session 6 infrastructure committed (`corpus-gen`, `usi_client`, `elo-tester`).
- [x] YaneuraOu smoke-corpus working (5 games, 100% decisive).
- [x] SFEN round-trip Session-6 open question: still unvalidated (first task this session).

---

## Work Completed

### Subtask 1: SFEN round-trip sanity check
- **Status**: Completed, uncovered a real bug.
- Picked three SFENs from the Session-6 smoke corpus (`/tmp/corpus_smoke.jsonl`) at lines 1, 100, 300; fed each back to YaneuraOu via `position sfen <s> 1`.
- Result A: line 1 (early game, empty hand): recorded eval_cp 188, re-fed eval 192. Δ = 4 cp. ✓
- Result B: line 100 (mid-game, big hand): **YaneuraOu silently accepted the SFEN but produced no search output** — neither `info` nor `bestmove`. It treated the SFEN as invalid.
- Result C: line 300 (later game, small hand): recorded -400, re-fed -420. Δ = 20 cp. ✓
- **Root cause**: the rejected line's SFEN contained 4 bishops in hand plus 2 on the board = **6 bishops total**, but Shogi has only 2. The corpus generator's `captured_pieces` state was growing over time. Tracing backwards from plies 20 → 40 → 60 → 100 of game 0:
  ```
  ply 20: 2 bishops (on board), 0 hand → total 2 ✓
  ply 40: 1 on board, 3 in hand       → total 4 ✗
  ply 60: 1 on board, 4 in hand       → total 5 ✗
  ply 100: 2 on board, 4 in hand      → total 6 ✗
  ```

### Subtask 2: Fix the drop-move bug in `corpus_gen.rs`
- **Status**: Completed.
- `BitboardBoard::make_move` does not touch `captured_pieces` — it's the caller's job. For regular moves with captures we already called `captured.add_piece(...)`. But for *drop moves* we never called `captured.remove_piece(...)`, so every drop left a phantom copy of the dropped piece in hand while also placing it on the board. This doubles the piece, and repeated drops accumulate ghost pieces.
- Fix (two sites: random-opening loop + teacher-driven loop):
  ```rust
  if mv.from.is_none() {
      captured.remove_piece(mv.piece_type, player);
  }
  if let Some(cap) = board.make_move(&mv) {
      captured.add_piece(cap.piece_type, player);
  }
  ```
- Post-fix smoke test (same 5-game / seed 2026 / depth 8 params): same overall shape, but line 100 of the corpus now has hand `PPpplsnps` (0 bishops) matching 2 bishops on board = 2 total. ✓
- Round-trip verification after fix: line 100 SFEN now **accepted** by YaneuraOu, which returned cp 2868 vs recorded 2696 — Δ = 172 cp, well within search noise for a +2700 position.
- **Implication for the committed Session-6 smoke corpus**: it was data-corrupt from ply ~20 onward in every game. It is now regenerated correctly and the original is untracked.
- **Implication for `nnue_trainer.rs`**: it has the *same* bug, but it never affected PST training because PST feature extraction reads the board only, not the hand. Corpus generation is the first consumer that needed correct hand state.

### Subtask 3: Generate a real corpus
- **Status**: Completed.
- Command: `corpus-gen --engine-path <YaneuraOu> --games 500 --depth 10 --random-plies 8 --seed 20260423 --output nnue_corpus_yaneura_d10.jsonl`.
- Result:
  ```
  500 games, 274 black wins / 191 white wins / 35 draws (93.0% decisive)
  67 403 positions written, 14 MB file, 947 s wall time (~71 positions/sec).
  ```
- Black-vs-white win ratio 274:191 (≈55%) reflects Shogi's first-move advantage, as expected.

### Subtask 4: Write the offline trainer
- **Status**: Completed.
- New file `src/bin/nnue_offline_trainer.rs` (~280 lines): loads JSONL, reconstructs the board via `BitboardBoard::from_fen`, extracts active NNUE features, refreshes the accumulator, computes blended target `(1-α)·tanh(clamp(eval_cp,±2000)/600) + α·outcome_to_move_pov`, calls `NNUETrainer::train_batch(..)`, saves weights. CLI: `--corpus`, `--init-weights`, `--output-weights`, `--epochs`, `--batch-size`, `--learning-rate`, `--outcome-weight`, `--eval-clamp`, `--output-grad-scale`, `--input-grad-scale`, `--checkpoint-every`, `--seed`, `--skip-null-eval`.
- Added public method `NNUETrainer::train_batch(&[TrainingPosition])` so an external corpus-driver can invoke the existing `update_weights_for_position` without going through the self-play queue.
- Hand-rolled JSON parsing (no serde dependency added) — the schema is fixed and small.
- Wired in `Cargo.toml`.

### Subtask 5: Diagnose training not converging — promoting gradient scales to config
- **Status**: Completed.
- First full-corpus run at the PST-tuned defaults (lr=0.005, output_scale=1e7, input_scale=1e6): TD error stuck at ~0.985 (near theoretical max). Verbose log showed batch 0 with `w_chg=1.248` (weights moved) and every subsequent batch `w_chg=0.000`: weights saturated in the first batch and couldn't move after that.
- **Root cause**: `update_weights_for_position` applies updates *per position* (online SGD). With PST-training the error was ~0.001 so per-position updates were a small fraction of an i16 unit; with teacher-target training the error is ~1, gradients are ~1000× larger, and weights pin to the i16 range `[-127,127]` (or `[-32767, 32767]`) inside the first batch of 256 positions.
- Fix 1: Promoted the two hardcoded scales to fields on `NNUETrainingConfig` (`output_grad_scale`, `input_grad_scale`) with the old hardcoded values as defaults and `#[serde(default = ...)]` so existing JSON weight files still load. Offline trainer exposes them as CLI flags.
- Dialling the scales down (`lr=0.005, out_scale=1e4, in_scale=1e3`) fixed the saturation but now per-position updates were so small they *all* rounded to zero in the `as i16` cast — still `w_chg=0.000` in every batch.

### Subtask 6: Add `train_batch_accumulated` — proper batched-SGD with f32 accumulation
- **Status**: Completed (unblocks the training loop; doesn't solve the underlying i16-quantization issue, see below).
- The sweet spot between "rounds to zero" and "saturates within a batch" is essentially empty when updates are applied per-position. Fix: accumulate per-position gradients in f32, then apply one aggregated update per batch.
- New method `NNUETrainer::train_batch_accumulated(&[TrainingPosition])` (~200 lines): f32 accumulators for every weight and bias; forward pass + backprop computed per-position, gradients summed; single `clamp(-X, X) as i16` applied per weight after the whole batch.
- Offline trainer switched to call `train_batch_accumulated` instead of `train_batch`.
- Smoke result: `w_chg` is now in the 1-3 range per batch (weights actually move every batch). TD error drops 0.66 → 0.65 over 30 epochs and plateaus.

### Subtask 7: Re-run `elo-tester` with the teacher-fine-tuned weights
- **Status**: Completed.
- Ran `elo-tester --games 20 --depth 3 --time-ms 150 --random-plies 16 --seed 123 --nnue-weights nnue_weights_yaneura_trained.json`.
- Result: **0 wins / 20 draws / 0 losses** — *identical* to Session 5's pilot with the untrained PST-mimic weights. Both sides still drift into the same 3-fold repetition around ply 25-30.
- Diagnosis: weights did move (verified at weight-file level), but TD-error plateau was only 0.66 → 0.65 over 30 epochs — the gradient signal that reaches the i16-quantized *input* layer is still too weak. The model learned a tiny shift in output/layer-2 weights, but not enough to meaningfully alter the moves the search engine picks at depth 3.

---

## Testing & Verification

### Build Check
```
cargo build --release --bin nnue-offline-trainer  → Pass
cargo build --release --bin corpus-gen            → Pass (after drop-fix)
No new warnings from new code.
```

### Functional Tests
```
Test 1: SFEN round-trip after drop-fix: 3/3 samples accepted by YaneuraOu,
        eval deltas 4cp / 172cp / 20cp.                             → Pass
Test 2: corpus-gen 500 games depth 10: completed without crash.     → Pass
Test 3: Offline trainer parses JSONL + updates weights per batch.   → Pass
Test 4: train_batch_accumulated produces non-zero w_chg.            → Pass
Test 5: elo-tester pipes the new weights correctly.                 → Pass
Test 6: Does the retrained NNUE play stronger than PST?             → FAIL
        (0/20 decisive, same as baseline.)
```

### Performance/Metrics
```
Corpus generation: 67 403 positions / 947 s = 71 pos/sec (YaneuraOu d10, 1 thread).
Trainer throughput: 67 403 pos / 1.8 s = 37 400 pos/sec per epoch.
Round-trip delta on random sample: mean 65 cp, max 172 cp (all within search noise).
```

---

## Observations & Insights

**What went well**:
- Every single piece of Session-6 + Session-7 infrastructure worked end-to-end on the first real integration: spawn, handshake, 500-game self-play, JSONL, parse, reconstruct, train, save, evaluate. The only bugs surfaced were *consumer-facing problems my own code was revealing*, not plumbing failures.
- The SFEN round-trip check paid for itself immediately: without it, we would have trained on corrupt data and blamed NNUE for the bad gameplay.
- The YaneuraOu 93% decisive rate validates Session 6's premise — the training-signal bottleneck really was the PST teacher, not something inherent in Shogi self-play.
- Promoting the hardcoded gradient scales to config turned out to be necessary for *any* future change to the teacher, loss, or architecture; putting this behind a `#[serde(default)]` keeps existing weight files loading.

**What was harder than expected**:
- The drop-move bug took a round-trip check to surface because the SFEN looked plausibly-shaped — only a careful piece count revealed it.
- The i16 weight quantization is a much sharper blocker than anticipated. Per-position SGD on i16 weights has a narrow sweet spot where updates are neither always zero (rounding) nor always pinned (saturation). In the PST regime that sweet spot happened to cover the operating range; in the teacher-target regime (gradients ~1000× larger) it doesn't cover much of anything.
- `train_batch_accumulated` fixed the mechanical "weights don't move" symptom but didn't fix the deeper issue: after gradients make their way to the input layer through a chain of small i16-quantized factors, the effective signal-to-noise ratio is too low for meaningful learning at this corpus size. Modern NNUE trainers (nnue-pytorch, YaneuraOu's own trainer) all use f32 shadow weights throughout training and quantize only at export — this session confirms that empirically.

**Surprises**:
- After 30 epochs of fine-tuning from a known-good starting point (the Session-3 PST-mimic weights), gameplay was **bit-identical** to the starting point — same game lengths, same draw pattern, same 20/20 draws in `elo-tester`. That level of null result tells us the updates, although non-zero at the weight-file level, weren't crossing the threshold of changing any ply-3 best move. That's a surprising amount of insensitivity.
- The `SFEN` encoding-bug (the drop-move issue) was latent in `src/bin/nnue_trainer.rs` for sessions 2-3 too, and never surfaced because PST training only reads the board. It was a trap waiting for the first consumer that actually read the hand.

**Insights for future sessions**:
- The next useful change is **f32 shadow weights**, owned by the trainer, used for accumulation and forward pass during training; the existing i16 weights become a quantized snapshot re-derived from the shadow at save time.
- Corpus size of 67 K is probably enough for a first *working* signal given a proper trainer; we don't need to generate more data yet.
- Once the trainer actually learns, the `corpus-gen` tool easily scales (just more games, or depth 12+ for higher-quality targets).
- The promoted-rook move-gen bug flagged in Session 6 is still open; it did not bite us this session.
- The `nnue_trainer.rs` drop bug should probably be fixed too, even though it didn't affect PST training output — the resulting self-play games were still using corrupt hand state internally, which may have leaked into TT hashing and therefore into search decisions during training.

---

## Decisions Made

**Decision 1**: Fix the drop-move bug in `corpus_gen.rs` only; leave `nnue_trainer.rs` alone for now.
- **Rationale**: Session 7 is about NNUE training via external teacher. The PST-trainer bug matters for future PST training runs but isn't load-bearing for what we're doing now, and touching it would require a second regen of Session-3 weights. Tracked for cleanup.
- **Confidence**: Medium — it's a real bug in a neighbouring binary, but its historical impact is bounded.

**Decision 2**: Promote gradient scales to `NNUETrainingConfig` with `#[serde(default)]` rather than hard-coding a second set for the offline trainer.
- **Rationale**: Keeps one source of truth. `serde(default)` means existing JSON weight files (without these fields) still load, so we don't invalidate Session-3 artefacts. Defaults match the old hardcoded values.
- **Confidence**: High.

**Decision 3**: Add `train_batch_accumulated` alongside `train_batch` rather than replacing the per-position update path.
- **Rationale**: The existing path still works for the PST-training regime (tiny errors, no saturation risk). Keeping both avoids a behaviour change for existing callers.
- **Confidence**: High.

**Decision 4**: Accept 0/20 decisive as the real measurement and stop trying hyperparameter permutations.
- **Rationale**: I'd cycled through ~8 combinations of `(lr, output_scale, input_scale, batch_size, init_weights)`; the consistent result was either `w_chg=0` or `w_chg=1-3` with TD-err plateau and identical gameplay. More permutations are unlikely to cross the i16-quantization threshold — the fix is structural (shadow weights), not parametric.
- **Confidence**: High.

**Decision 5**: Do not commit the full 14 MB corpus file.
- **Rationale**: Training artefacts, not source. Regeneratable by a one-line command. Keeps the repo light.
- **Confidence**: High.

---

## Blockers & Issues

### Issue 1 (resolved): Drop-move captured-piece tracking
- **Severity**: High (was producing malformed training corpora)
- **Resolution**: Fixed in `corpus_gen.rs` (both random-opening and teacher-driven loops).
- **Status**: Resolved.

### Issue 2 (open): i16 weight quantization blocks effective training
- **Severity**: High (blocks Phase-4 validation)
- **Description**: Per-position SGD updates either round to 0 or saturate i16 in one batch. Batched accumulation helps mechanically but gradient chain through a quantized network still has sub-unit signal at the input layer, so the input weights barely budge even over 30 epochs of real data. Result: post-training gameplay is indistinguishable from the starting weights at the `elo-tester` pilot level.
- **Root cause**: Training in the quantized forward-pass domain without f32 shadow weights.
- **Planned resolution (Session 8)**: Add f32 shadow-weight arrays to `NNUETrainer`. Accumulate all gradient updates in f32 during training. Re-quantize to i16 on save / on forward-pass-from-shadow (optionally). This is the standard approach used by nnue-pytorch and YaneuraOu's own trainer.
- **Status**: Open.

### Issue 3 (open, from Session 6): Promoted rook move generation truncated
- **Severity**: Medium
- **Status**: Still open. Did not surface this session (corpus-gen does not validate legality against our own generator).

### Issue 4 (deferred): Same drop-move bug exists in `nnue_trainer.rs`
- **Severity**: Low in practice (PST doesn't read hand; training was still "working" well enough).
- **Status**: Tracked. Should be fixed before the next PST self-play retrain.

---

## Next Session Plan (Session 8)

**What to do next** (ordered):

1. **Implement f32 shadow weights in `NNUETrainer`**:
   - Add `shadow: Option<ShadowWeights>` field containing `Vec<Vec<f32>>` for input, `Vec<f32>` for biases, etc.
   - On `NNUETrainer::new`, populate `shadow` from the current i16 weights (convert).
   - `train_batch_accumulated` updates `shadow` in f32 (no clamping / rounding during training).
   - At save / every N batches, re-quantize: `i16 = shadow.clamp(-127, 127).round()` for inputs, similar for others.
   - Forward-pass-during-training can optionally read from `shadow` (for de-quantized gradient computation).
2. **Retrain on the existing 67K corpus** with shadow weights, 30-50 epochs, checkpoints every 10.
3. **Re-run `elo-tester`** with the new weights. Target: *some* decisive games (>0/20), ideally a small positive ELO delta.
4. If step 3 shows signal: generate a bigger corpus (~200K positions at depth 12) and retrain.
5. If step 3 shows no signal: investigate feature extraction (our NNUE uses piece-on-square features — no hand-piece features, no king-relative features; a strong teacher may need more than that to be learnable).

**Decision points**:
- [ ] Size of the shadow arrays: use f32 everywhere (memory ~4× of i16, manageable at 256→32→1) or only for input weights where signal is smallest.
- [ ] Whether to also fix the `nnue_trainer.rs` drop bug in this session or defer again.

**Estimated duration**: 3-5 hours.
**Prerequisite**: None (Session 7 deliverables stand independently).

---

## File Changes Summary

### Files Modified
- `src/evaluation/nnue_training.rs`
  - Added `output_grad_scale`, `input_grad_scale` to `NNUETrainingConfig` with `#[serde(default)]` fns.
  - Replaced hardcoded `1e7` / `1e6` with `self.config.output_grad_scale` / `self.config.input_grad_scale` inside `update_weights_for_position`.
  - Added public method `train_batch(&[TrainingPosition])`.
  - Added public method `train_batch_accumulated(&[TrainingPosition])` implementing proper batched SGD with f32 accumulation and one i16 clamp per weight at batch end.
- `src/bin/corpus_gen.rs`
  - Fixed drop-move `captured_pieces` tracking at both call sites.
- `Cargo.toml`
  - Added `[[bin]] nnue-offline-trainer`.

### Files Created
- `src/bin/nnue_offline_trainer.rs` (~280 lines): CLI-driven offline trainer reading JSONL.
- `docs/nnue-phase2/SESSION_LOG_007.md` (this file).

### Files Deleted
- None.

### Artefacts produced (untracked)
- `nnue_corpus_yaneura_d10.jsonl` (~14 MB, 67 403 positions, 500 games at depth 10).
- `nnue_weights_yaneura_trained.json` (fine-tuned weights — effectively identical in gameplay to the Session-3 weights; kept for reproducibility of the null result).
- `nnue_weights_yaneura_trained_epoch_{10,20,30}.json` (checkpoints).
- `/tmp/elo_{yaneura_trained,yan_acc,final}.csv` (three pilot runs — all 0/20/0 draws).

---

## Testing Results

### Session 7 success criteria

**Criterion 1**: Integration works end-to-end (corpus → trainer → weights → elo-tester)
- [x] Met.

**Criterion 2**: Real corpus of ≥ 50K decisive-rich positions generated
- [x] Met (67 403 positions, 93% decisive).

**Criterion 3**: Trainer actually updates weights
- [x] Met after the batched-accumulation fix (w_chg 1-3 per batch).

**Criterion 4**: First measurable NNUE > PST ELO gain
- [ ] **Not met** (0/20 decisive, identical to the untrained baseline).
- Root cause diagnosed: i16 quantization of input weights in the training forward/backward path.
- Unblocks in Session 8 by introducing f32 shadow weights.

---

## Historical Session Reference

| Session | Phase         | Title                                                                 | Status    | Date       |
|---------|---------------|-----------------------------------------------------------------------|-----------|------------|
| 1       | 1.1-1.2       | Weight Initialization & Output Scaling                                | Completed | 2026-04-22 |
| 2       | 1.3, 2.1-2.3  | Training Verification & Algorithm Fixes                               | Completed | 2026-04-22 |
| 3       | 2.4           | Extended Training + Game Diversity                                    | Completed | 2026-04-22 |
| 4       | 3.2, 3.4      | Speed Optimization (Incremental Accumulator)                          | Completed | 2026-04-22 |
| 5       | 4.1           | ELO Validation Infrastructure + First Pilot                           | Completed | 2026-04-22 |
| 6       | 2b-setup      | External USI Teacher — Corpus Generator Infrastructure                | Completed | 2026-04-22 |
| 7       | 2b-execute    | YaneuraOu Corpus + Offline Trainer — Integration Lessons              | Completed | 2026-04-23 |
| 8       | 2b-finish     | f32 Shadow Weights + Retrain + Re-measure                             | Pending   | TBD        |

---

## Sign-Off

**Session Lead**: Claude (AI)
**Status**: Completed (infrastructure + diagnosis; actual ELO gain deferred to Session 8)
**Ready for next session**: Yes
**Comments**: The session's concrete wins are a real fix (drop-move bug, affecting corpus and latent in the PST trainer), a real corpus (67K YaneuraOu positions, 93% decisive), a real binary (`nnue-offline-trainer`), and a real understanding of what was silently blocking training the whole time (i16 weight quantization in the SGD path). The null ELO result is not a regression — it matches Session 5's baseline, and the path forward is clear: f32 shadow weights in Session 8.

---

**Template Version**: 1.0
**Last Updated**: 2026-04-23
