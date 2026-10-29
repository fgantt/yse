# NNUE and search development log

One log per working session, from the first NNUE experiments to the current engine. **Start
with the latest log's READ THIS FIRST block**; it states what is current. Older logs keep their
original conclusions, and several were later corrected (later logs say so).

- `FEATURE_SPEC_AND_AUDIT.md`: feature encoding specification and audit (2026-09-17).
- `SESSION_LOG_TEMPLATE.md`: template for a new log.
- `run.sh`, `patches/`: scripts and patches cited by individual sessions.
- The April 2026 phase-2 plan that started this work is in `docs/archive/nnue-phase2-plan-2026-04/`.

| Session | Date | Title |
|---|---|---|
| [1](SESSION_LOG_001.md) | 2026-04-22 | Phase 1.1 + 1.2 - Weight Initialization & Output Scaling |
| [2](SESSION_LOG_002.md) | 2026-04-22 | Phase 1.3 + Phase 2.1-2.3 - Training Verification & Algorithm Fixes |
| [3](SESSION_LOG_003.md) | 2026-04-22 | Extended Training Verification + Game Diversity Fixes |
| [4](SESSION_LOG_004.md) | 2026-04-22 | Phase 3 - Speed Optimization (Incremental Accumulator + Forward Pass) |
| [5](SESSION_LOG_005.md) | 2026-04-22 | Phase 4 - ELO Validation Infrastructure + First Head-to-Head Match |
| [6](SESSION_LOG_006.md) | 2026-04-22 | External USI Teacher - Corpus Generator Infrastructure |
| [7](SESSION_LOG_007.md) | 2026-04-23 | YaneuraOu Teacher Corpus + Offline Trainer — Integration Lessons |
| [8](SESSION_LOG_008.md) | 2026-04-24 | f32 Shadow Weights — Mechanics Fixed, Reveals Deeper Structural Ceiling |
| [9](SESSION_LOG_009.md) | 2026-04-24 | Relaxed Teacher Target + elo-tester Fix — Output-Range Ceiling Breaks |
| [10](SESSION_LOG_010.md) | 2026-04-25 | Symmetric Output-Layer Init — Negative Result, Reframes the 0.47 Floor |
| [11](SESSION_LOG_011.md) | 2026-04-25 | Lower the Output Mapping Divisor — Mechanically Correct, Reveals the Real Floor |
| [12](SESSION_LOG_012.md) | 2026-04-26 | Three Diagnostics for Input-Layer Starvation — All Three Negative |
| [13](SESSION_LOG_013.md) | 2026-04-26 | Sigmoid+MSE Loss — Eval-Path Bug Falsified, Sigmoid Loss Negative |
| [14](SESSION_LOG_014.md) | 2026-04-26 | Side-to-Move Feature — Structural Intervention Lifts Pearson r From Noise Floor To +0.59 |
| [15](SESSION_LOG_015.md) | 2026-04-27 | Adam Optimiser + Extended ELO Bake-Off — Adam Matches SGD From S3 Init, **Beats It From Fresh Init (+0.61 r)**, ELO CI Tightens |
| [16](SESSION_LOG_016.md) | 2026-04-28 | HalfKP Feature Engineering + Adam-Fresh-Init ELO Confirmation — HalfKP Beats Session 15 Ceiling **+0.61 → +0.614 r** (40% lower td_err); Adam-Fresh ELO Confirms **+66 ± 52 over 90 cumulative games** (CI95 [+16, +120]) |
| [17](SESSION_LOG_017.md) | 2026-04-29 | Engine-Side HalfKP + ELO Confirmation Bake-Off — HalfKP Reaches **+120.4 ± 76.8 ELO** (CI95 [+43.6, +197.2]) Over PST in 30 Games (11W-18D-**1L**), Strongest NNUE Result of Phase 2 |
| [18](SESSION_LOG_018.md) | 2026-04-29 / 2026-04-30 (bake-off ran across midnight UTC) | Head-to-Head HalfKP vs Flat-NNUE + Binary Weights Format — HalfKP **+23.2 ± 72.7 ELO** Over Flat-NNUE (CI95 [-49.5, +95.9]) in 30 Games (6W-20D-4L); Binary Format Lands at **4.57× Shrink, 7.7× Faster Load** |
| [18 (Evaluation)](SESSION_LOG_018_Evaluation.md) |  | Evaluation |
| [19](SESSION_LOG_019.md) | 2026-05-01 | Pieces-in-Hand Thermometer Encoding + Trainer `.bin` Default + Weight-File Cleanup — 11/11 NNUE Tests Pass; Fresh-Init Hand-Feature Network Reaches **r_blk = +0.610, r_wht = +0.566** in 10 Epochs (Per-Stratum Parity with S15 Baseline at 1/3 the Epoch Count) |
| [20](SESSION_LOG_020.md) | 2026-05-01 (training + diagnostic) / 2026-05-02 (bake-off completion) | Mode-Independent Pearson Diagnostic + 30-Epoch Hand-Feature Retrain + Two Head-to-Head Bake-Offs — Diagnostic Lifted, Both Bake-Offs Inconclusive (CIs Bracket Zero) |
| [21](SESSION_LOG_021.md) | 2026-09-17 | Feature-Space Audit — The Plateau Was the Metric; Perspective Normalisation Lifts Held-Out r From 0.449 to 0.641 |
| [22](SESSION_LOG_022.md) | 2026-09-18 | Search Harness and Search Audit — NNUE Has Never Chosen a Move |
| [23](SESSION_LOG_023.md) | 2026-09-18 | Material PSQT Path — NNUE Beats Classical |
| [24](SESSION_LOG_024.md) | 2026-09-18 – 2026-09-19 | Modern Architecture — Two Perspectives, King Buckets, and a Trustworthy Bake-off |
| [25](SESSION_LOG_025.md) | 2026-09-19 | Search and Time Management — and Where the Strength Actually Goes |
| [26](SESSION_LOG_026.md) | 2026-09-19 – 2026-09-20 | Training on Our Own Games — +65 Elo Against Ourselves, Nothing Against the Teacher |
| [27](SESSION_LOG_027.md) | 2026-09-20 | The Search Was Throwing the Evaluation Away |
| [28](SESSION_LOG_028.md) | 2026-09-21 – 2026-09-22 | 100K Games, and Finding Out What We Were Measuring |
| [29](SESSION_LOG_029.md) | 2026-09-23 | Training on Move Ranking |
| [30](SESSION_LOG_030.md) | 2026-09-24 to 2026-09-27 | Nodes Per Second |
| [31](SESSION_LOG_031.md) | 2026-09-27 to 2026-09-28 | Four Bugs That Did Not Matter, and One Blind Spot That Does |
| [32](SESSION_LOG_032.md) | 2026-09-30 to 2026-10-07 | Idle Threads, Fake Draws, and a Gate That Can Decide |
| [33](SESSION_LOG_033.md) | 2026-10-07 to 2026-10-08 | Reductions at Split Nodes (rejected) |
| [34](SESSION_LOG_034.md) | 2026-10-08 | Exact Speed-ups (single-threaded +51%) |
| [35](SESSION_LOG_035.md) | 2026-10-14 | Speed work, and our node counts were wrong |
| [36](SESSION_LOG_036.md) | 2026-10-17 | Twice, then three times the even-game data: p9 (+100), then q9 (+173 vs p9) shipped |
| [37](SESSION_LOG_037.md) | 2026-10-22 | Search check (evaluation ~half the gap, no single pruning rule); data batches 4 and 5: r9 (+35 vs q9), then s9 (+80 vs r9) shipped; batch 6 +28 (inconclusive) and depth-10 labels no better than depth 8; next: research before choosing |
| [38](SESSION_LOG_038.md) | 2026-10-28 | A PyTorch GPU trainer: the old trainer's integer-rounded input layer was the limit; g5, trained from scratch on the same 375M positions, shipped (+93 vs s9) |
