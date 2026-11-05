# Handoff to Session 40: step back, research first

**Written**: 2026-11-05, end of Session 39.
**Decision (user, 2026-11-05)**: option (b). **The research is the primary and most important
work.** The 20%-share Hao mix runs on the otherwise idle PC as a cheap side job; nobody should
count on it.

## 1. Why we are stepping back

a1 (`nnue/default.bin`, shipped 2026-11-01) is our strongest engine, yet it still loses every
standard (even) game the user plays it against other engines (YaneuraOu-class). It reaches deep
into games and still loses. Data-side levers are giving out:

| Change (all vs a1 unless noted) | Result |
|---|---|
| h4 → quiet → m8 → a1 (quiet filter, then Hao data) | +64, ~+100-150, +111 at the 1 s gate |
| c1, c2: + our Stage B self-play (110M, 218M quiet) | c1 about +7/+11; c2 below a1 |
| c1m, c2m: middle layer (`--mid 32`, v6) | no measurable difference |
| c1s: Stage B relabelled by Suisho5, depth 8 | vs c1 −28, −14 (FAIL); vs a1 +3, −56 |
| h2: + 200 more quiet Hao files (2.06B, Hao 89%) | screens +25 (cap), +35 (PASS); **1 s SPRT +20.9 ± 25.4 over 600, INCONCLUSIVE** |

Data, teacher strength and a bigger network have all stopped paying at about the same time, so
the limit is probably somewhere we have not measured. Full numbers:
`docs/nnue-phase2/DATA_LINEAGE.md` (networks table and the Stage B / teacher-test notes).

## 2. Primary work: research and gap measurement

The user wants a survey across the field, careful reasoning and a checked plan **before** any new
direction or multi-day run (memory: research before choosing). Deliver a written comparison and
a ranked plan; get approval before acting on it.

### 2a. Survey of the field

What separates strong shogi engines from mid-strength ones, with sources, ranked by expected
size of gain for us:
- **Evaluation**: network architectures and feature sets beyond our HalfKP 256→32→1 (v5) /
  v6 middle layer: e.g. HalfKA/HalfKAv2-style features, wider accumulators, layer stacks or
  buckets, PSQT outputs, quantisation choices. What Suisho5, Hao, tanuki-, Fairy-Stockfish and
  Stockfish-derived shogi nets use, and what each change was worth when introduced.
- **Training**: data scale (positions, depth of labels), recipes (loss, lambda/outcome weight,
  learning-rate schedule, epochs), filtering, and how published nets were trained.
- **Search**: the features and tuning that matter most in YaneuraOu-family and Stockfish-family
  search (pruning, extensions, history tables, time management), and speed (nps per thread,
  Lazy SMP scaling).
- Rules: study ideas only; no code, constants or networks of YaneuraOu (or others) — memory
  `no_copying_from_yaneuraou`. Published licensed data is fine. Ask before every download
  (filename, source, size).

### 2b. Measure our gap to YaneuraOu by part

Use the established tools (CLAUDE.md, "Trustworthy measurements"):
- **Node-equivalence**: fixed-node matches vs YaneuraOu at several node ratios (e.g. us at 1x,
  2x, 4x, 8x its nodes) to express the gap as "how many more nodes we need". Remember USI node
  counts exclude quiescence (memory `node_counts_exclude_quiescence`; use `search_bench`
  all_nodes for speed).
- **Eval quality at equal depth**: neutral-set move loss (150 held-out teacher positions) and
  `scripts/eval_gap.py`; a diagnostic, not a target.
- **Tree shape**: `search-diag` per-ply node ratio, cutoff index, LMR rejections vs earlier
  sessions.
- **The user's lost games** (`matches/user_games/`, incl. the 2026-11-01 a1 vs YaneuraOu
  AVX512VNNI game and its analysis): for each loss, find the losing moves and classify them as
  eval misjudgement (the eval at depth was wrong) or search miss (the right move is found by our
  eval given more depth/nodes). This says whether the next lever is evaluation, search or speed.
- Matches block the user: run long ones only in windows the user chooses; offline analysis first.

Output: a document (e.g. `docs/nnue-phase2/RESEARCH_S40.md`) with the survey, the measured gap
decomposition, and a ranked, costed plan. Present it before starting anything multi-day.

## 3. Side job on the PC: a1-share mix ("h3")

Question: did dropping our share from 20% (a1) to 11% (h2) cost part of what the new Hao gained?
Expected: +10 to +30 over a1 at best; a large gain is unlikely (<10%).

Recipe (not yet run; check before launching):
- Inputs: `~/data/ours_quiet` **listed twice** (2 × 222.6M) + `hao/packed_s50q` (229.8M) +
  `packed_s150q` (683.8M) + `packed_s200bq` (920.3M) → ~2.28B, ours ≈ 19.5%. `trainer/mix.py`
  opens each input separately, so listing a directory twice should simply interleave two copies;
  verify the counts in `mix_h3.log`. Its val.bin then holds ours' val twice — harmless, but
  note it. Disk: `~` had ~270 GB free before `mix_h2` (206 GB) was written; **delete an old mix
  first** (e.g. `mix_c2`, `mix_c1s`, `mix_a2`, `mix_q` — all reproducible from packed sets) and
  ask the user before deleting.
- Train exactly as p1_a1/p1_h2: `--epochs 8 --epoch-size n/4 --lr 1e-3 --gamma 0.7
  --outcome-weight 0.3` (~6.5 h at ~200k pos/s).
- Run it like `~/yse/stage_h2.sh` (restartable stages, `--resume`) driven by a copy of
  `corpus_s39/next_h2.sh` (held-open SSH with keepalives, retries; SSH dropped twice this
  session after ~5 h). Then `matches/s39/pc_screen.sh p1_h3 SEED weights/session39/p1_a1_e008.bin va1`
  and, if it passes, also screen vs p1_h2. Ship gate only on a clear screen result, with the
  user's go-ahead for the Mac time.
- Add rows for `mix_h3` and p1_h3 to `DATA_LINEAGE.md`.

## 4. State at handoff

- **Shipped**: a1 (`nnue/default.bin`, sha256 e842ac27800d…). GUI uses `engines/usi-engine-stable`.
- **Best unshipped**: p1_h2 epoch 8, `weights/session39/p1_h2_e008.bin` (ec6d1a806f99…),
  about +20 over a1 at 1 s, not proven.
- **Nothing running** on the Mac or PC.
- **Uncommitted** (commit only when the user asks): `docs/nnue-phase2/DATA_LINEAGE.md`,
  `docs/nnue-phase2/hao/sample200b.txt`, this file, `SESSION_LOG_039.md` (pointer here), `scripts/label_games.py` (`LABEL_OPTIONS`),
  `src/bin/nnue_pack.rs` (`--eval-scale`). The h2 driver scripts (`corpus_s39/next_h2.sh`,
  `matches/s39/sprt_h2.sh`) are in ignored directories.
- **SESSION_LOG_039** still lacks c1, c1m, c2, c2m, c1s, h2, the a1 user game and batches 3–4;
  `DATA_LINEAGE.md` has all of them.
- **PC**: `matches/s39/pc.sh` (`run`/`put`/`get`; SSH `fgant@100.122.46.127` lands in WSL,
  commands on stdin; copied from the old session's scratchpad). `matches/s39/pc_screen.sh` uses it. Raw Hao files
  (400) in `/mnt/c/Users/fgant/hao/raw/`; packed sets in `/mnt/c/Users/fgant/hao/` and `~/data/`.
- Standing rules: CLAUDE.md and the memory index (no `git clean -x`, no `cargo fmt`, private
  engine copies, `SHOGI_PREFS_DIR`, screen before SPRT, hold the Mac awake with a renewing
  `caffeinate -u` loop for long jobs).
