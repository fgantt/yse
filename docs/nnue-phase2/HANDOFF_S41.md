# Handoff to Session 41: search is the lever

**Written**: 2026-11-09, end of Session 40. Detail: `docs/nnue-phase2/RESEARCH_S40.md` (survey,
gap measurements, council, every gate). Data and networks: `DATA_LINEAGE.md`.

## 1. State

- **Update 2026-11-10 (Session 41): shipped the council code-review fixes** (one commit on top of
  `506a366`, network a1 unchanged): legal-only quiescence moves, the PV-window overflow, quiescence
  repetition and cache horizon, fail-soft root, correction-history and killer fixes, hand repetition
  across the root, tablebase off in games, pondering per protocol, `USI_Hash` honoured (192 MB at
  256), and a fingerprint-identical speed batch. Council (Opus, Sol, Gemini) 8.5 / 8.2 / 9.5 after
  four rounds; gate SPRT [−20, +10] PASS, 864 games, +7 ± 22 (Mac −5 ± 25, PC +52 ± 50). Detail:
  `RESEARCH_S40.md`, "Council (2026-11-09, code review ...)" and after. Installed as
  `engines/usi-engine-stable` (previous: `-stable-prev` = `4dfe9c4`). The rest of this section
  describes the state before it.

- **Shipped engine: `4dfe9c4`** (root stack, `SHOGI_ROOT_STACK=63` default) with network **a1**
  (`nnue/default.bin`, unchanged since 2026-11-01). Installed as `engines/usi-engine-stable`
  (previous: `-stable-prev` = `8934982`), which the Mac GUI runs.
- `main` = `origin/main` at `908649b` (plus this handoff's doc edits, if committed).
- The Mac prefs file was reset to 16 threads on 2026-11-09 (a `game_gap.py` ladder run without
  `SHOGI_PREFS_DIR` had left it at 1; fixed in `908649b`).

## 2. Session 40 results

| Change | Gate (pooled two-seed SPRT [0, 40], 1 s, 4 threads unless noted) | Shipped |
|---|---|---|
| `8934982`: helper tables kept across moves, correction history, SPSA values | 112-5-76, LLR +2.97 PASS (raw ~+65, likely ~+40) | yes |
| Partial iterations for the main thread (patch: `matches/s40/partial_iter.patch`, local) | 179-13-180, ~0, LLR −3.12 FAIL | no |
| `4dfe9c4`: root stack (root LMR, `lean_hist` root ordering, root TT flag, helper aspiration windows, corrected q stand-pat, parked QTABLE) | 86-8-55, ~+73, LLR +3.05 PASS (offline estimate was +15-40) | yes |
| N4: `p1_s5`, a1's mix with Suisho5 depth-9 labels in place of Hao | PC screen, 250 ms, vs a1 on `4dfe9c4`: 398-20-402, ~0, LLR −6.02 FAIL | no |

**What the session established:**
- **The network is not the constraint now.** N1: a1 inside our search is as accurate as
  YaneuraOu's halfkpvm inside our search (Pearson vs YaneuraOu depth 14, 3,000 positions);
  with halfkpvm in our search, the 11-08 game's mistakes repeat. N4: a teacher worth 110-200
  Elo inside YaneuraOu changed our network by nothing measurable. Not indicated now: more king
  buckets (N3), the tanuki- 2024 set, more Hao.
- **Search gains are real and transfer to the gate.** Both passes this session were search.
  The root stack's nodes-to-depth cut (0.45-0.65x on four middlegames) predicted it better than
  fixed-node accuracy, which cannot see even a doubling of nodes.
- **USI node counts include quiescence since `849b44a`** (council, checked). §4 A's "16x" was
  ~16x YaneuraOu's work (memory `node_counts_exclude_quiescence` has the corrected note).

## 3. Open leads (none approved; research and state pass odds first)

From the council and follow-ups (`RESEARCH_S40.md`, "Council"):
1. **Speed is worth more than §0 claimed** (Claude, council): §4 A's table gives ~+80 per
   doubling. Test: stable at 1.6 s vs 1.0 s, 4 threads, ~300 games. Profile (mate pre-search
   excluded): NNUE eval 27.5%, legal movegen 14%, SEE 11%, q movegen 11%, malloc 5.6%.
   Fingerprint-identical speedups need no games: stack, then gate once.
2. **Helpers clone the 10.8 MB network every move** (`clone_for_worker`); share it by `Arc`
   (fingerprint-identical).
3. **SPSA v1 tuned five inert parameters** (`SHOGI_LEAN_HIST` = 1 and ProbCut off make
   `hist_prune_*`, `lmr_hist_div`, `probcut_*` dead). A rerun on live parameters, after the
   root stack changed the tree.
4. **Switched-off features** (singular extension, ProbCut, IIR, history LMR/pruning) need a
   fixed-node screen calibrated on known results before any gate.
5. Smaller: LMR excludes losing checks that pruning treats as quiet (`lean.rs:766` vs `:661`);
   the root stack's parts were not gated separately; a judge from another engine family for
   the game ladder (YaneuraOu judging itself biases N1's reading).

## 4. Machines and infrastructure

- **PC** (`matches/s39/pc.sh run|put|get`; SSH lands in WSL; commands on stdin). SSH was
  intermittent on 11-08/09: sessions hung or dropped for up to 45 min, and a 27 MB `get` broke.
  Keep files on the PC, use held-open reconnecting holders with `done` markers
  (`matches/s40/n4/hold_*.sh`, `s5_chain.sh`), and do not trust a silent session: check the
  remote log.
- **PC builds:** WSL's rustc is 1.79, too old (indexmap needs 1.82). Use Windows cargo 1.90:
  `/mnt/c/Users/fgant/.cargo/bin/cargo.exe` in `C:\Users\fgant\yse-s40` (source `4dfe9c4`,
  `SOURCE_COMMIT`; `usi-engine.exe`, `usi_match.exe`, `nnue-convert-weights.exe` built). Pass
  Windows paths (`wslpath -w`) and `WSLENV=SHOGI_PREFS_DIR/p`. `matches/s40/n4/screen_pc.sh` is a
  working PC screen; `matches/s39/pc_screen.sh` plays on the **Mac**.
- **PC disk (WSL home 324 GB free, C: ~106 GB free), N4 data kept, ask before deleting:**
  `/mnt/c/Users/fgant/suisho5/raw/` (184.6 GB, 8 verified 7z volumes),
  `/mnt/c/Users/fgant/suisho5/packed_s5` (91 GB), `~/data/mix_s5` (114 GB). `mix_h3` was deleted
  (rebuild: `mix.py` inputs in `DATA_LINEAGE.md`). Run `p1_s5` in `~/yse/trainer/runs/`.
- **Mac:** Suisho5 validation file `corpus_suisho5/val/shuffled.bin` (290 MB, ignored). N1
  scripts and raw output: `matches/s40/n1/` (ignored). Testbed (`SHOGI_YO_NNUE`) needs
  `--features yo-diagnostic`, or the variable is silently ignored.

## 5. Standing rules (unchanged)

CLAUDE.md and the memory index: no `git clean -x`, no `cargo fmt`; private engine copies and
`SHOGI_PREFS_DIR` for every engine launch (scripts included); ask before every download and
deletion; commit only when asked; screen before SPRT; matches block the user; before any
multi-hour run, write the four answers (effect vs gate power, state lifetime, test conditions,
fit to the gap) and state the pass odds. The user is tired of long tests that fail: prefer
offline measurements that take minutes, and say plainly when a test is unlikely to pass.
