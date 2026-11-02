# Training data lineage

Where every network's training data came from: games → labelled corpus → packed set → mix →
network. Started 2026-11-02 (Session 39) and filled in back to Session 35 from the generation
scripts, pipeline logs and training logs on both machines. Anything not verified is marked
**unknown**, never guessed. Counts are positions (records).

**Keep it current.** Add a row whenever a corpus is labelled, a set packed, a mix built or a
network trained; the `lineage.json` files below hold the facts to copy.

**Diagram:** `lineage.html` (beside this file; published at
https://claude.ai/artifact/PRSNaaj9JAUzvfmfSJbiZA) draws these tables as a swimlane timeline. Its
data is the `N` (nodes) and `E` (edges) arrays at the top of its script: add a node and its edges
there when you add a row here.

## Lineage files (since 2026-11-02)

- `nnue_pack pack` / `pack-psv` write `lineage.json` beside `train.bin`: inputs (absolute path,
  bytes), options (quiet, half, seed; for `pack-psv` the held-out file), counts, argv, date, host
  and `--note` (say what the input is, e.g. `--note "Mac batch 2 + PC batch 3"`).
- `trainer/mix.py` writes `lineage.json` with each input's path, train/val counts, limit and
  **that input's own `lineage.json` embedded**, so a mix carries its whole ancestry (`null` for
  sets packed before this existed: look them up below). Pass `--note`.
- `trainer/train.py` copies the data's `lineage.json` into the run as `data_lineage.json`; its
  `log.txt` already records every argument.

To trace a network: `runs/<run>/data_lineage.json`, else `args {...}` in `runs/<run>/log.txt`
(the `data` path), then the tables below.

## Conventions

- **Teacher:** YaneuraOu with the `halfkpvm_256x2-32-32` network on the Mac, depth 8
  (`scripts/label_stream.py`), labelling every position of every game. One teacher throughout,
  except Hao (labelled by its own publisher) and the depth-10 experiment.
- **Game generation:** even-game self-play, 4k vs 6k nodes, 8 random opening plies, unless noted.
- **Game ids** are unique across all our corpora (ranges below), so pools can be concatenated.
  Held-out games are every 20th game id (`nnue_pack`); in `pack-psv` the last input file.
- **Quiet filter** (`--quiet`): drops positions in check or whose next move (ours: the game move;
  Hao: the stored best move) captures. Keeps ~59-63% of ours, ~58% of Hao.
- Held-out losses are comparable only between runs that share a validation set (same data dir,
  or mixes of the same inputs) **and** the same outcome weight.

## 1. Labelled corpora (ours)

| Corpus (Mac) | Made | Games, game ids | Generation | Positions |
|---|---|---|---|---|
| `corpus_s35/gen_d8` (batch 1) | S35 | 583,687; 1,000,000+ | YaneuraOu self-play | 67.0M; 62.6M after dedupe (`corpus_s35/gen_d8_dedup.jsonl`) |
| `corpus_s36/gen_d8` (batch 2) | S36 | 583,604; 2,000,000+ | same, 14 × 41,686 games | 67.0M; 62.4M kept |
| `corpus_s36b/gen_d8` (batch 3) | S36 | 583,604; 3,000,000+ | same | 67.0M; 62.3M kept |
| `corpus_s37/gen_d8` (batch 4) | S37 | 583,604; 4,000,000+ | same | 67.0M; 62.3M kept |
| `corpus_s37b/gen_d8` (batch 5) | S37 | 588,000; 5,000,000+ | same, 14 slots × 14 runs × 3,000, fresh engines; seeds 410000+ | 67.5M; 62.7M kept |
| `corpus_s37c/gen_d8` (batch 6) | S37 | 588,000; 6,000,000+ | same; seeds 420000+ | 67.5M; 62.6M kept |
| `corpus_s37c/gen_d10` | S37 | batch 6's games | **depth-10 labels** (experiment, t10) | 62.6M kept |
| `corpus_s39/b1_d8` (Mac batch 1) | S39 | 540,000; 7,000,000-7,539,999 | 504k YaneuraOu (B1) + 36k m8 at 3k nodes vs YaneuraOu 6k (B2); seeds 510000+ | 61.3M |
| `corpus_s39/pc1_d8` (PC batch 1) | S39 | 504,000; 20,000,000+ | **PC's YaneuraOu = Suisho5 network**, B1 recipe, 12 × 14 × 3,000; seeds 610000+ | 58.7M |
| `corpus_s39/pc2_d8` (PC batch 2) | S39 | 504,000; 21,000,000+ | same; seeds 620000+ | 58.7M |
| `corpus_s39/b2_d8` (Mac batch 2) | S39 | 522,000; 8,000,000-8,521,999 | 504k B1 + 18k a1 at 10k nodes vs YaneuraOu 6k (B2); seeds 520000+ | 59.8M |
| `corpus_s39/pc3_d8` (PC batch 3) | S39 | 504,000; 22,000,000+ | as PC batch 1; seeds 630000+ | 58.7M |
| `corpus_s39/pc4_d8` (PC batch 4) | S39 | 504,000; 23,000,000+ | as PC batch 1; seeds 640000+ | 58.7M |

Batches 1-6 were deduplicated as they were pooled (first record of a repeated position kept,
`corpus_s36/dedupe_pool.py`), into cumulative pools: `corpus_s36/pool_d8.jsonl` (1-2, 125.0M),
`corpus_s36b/pool_d8.jsonl` (1-3, 187.2M), `corpus_s37/pool_d8.jsonl` (1-4, 249.5M),
`corpus_s37b/pool_d8.jsonl` (1-5, 312.2M), `corpus_s37c/pool_d8.jsonl` (1-6, 374,835,085) and
`pool_d10.jsonl` (1-5 at depth 8 + batch 6 at depth 10). **Stage B (S39) batches were not
deduplicated**, only concatenated. PC games are generated on the PC and labelled on the Mac.

Before Session 35: corpora `corpus_s23` … `corpus_s28`, `corpus_self`, `corpus_hc`,
`corpus_rank` fed networks up to s26 (14.0M positions, Session 26); see those session logs.

## 2. Published data

| Set | Source | Files | Positions |
|---|---|---|---|
| Hao sample 50 | nodchip `shogi_hao_depth9` (Hugging Face, MIT), depth-9 labels by its publisher | `docs/nnue-phase2/hao/sample50.txt` (name, size, sha256; spread over the set's 8 generation runs) | 397.5M |
| Hao sample 150 | same set | `docs/nnue-phase2/hao/sample150.txt` (all 150 packed; 2 were already on disk) | 1,166M |

Raw files: Mac `corpus_hao/raw/` (sample 50), PC `/mnt/c/Users/fgant/hao/raw/` (all 200).

## 3. Packed sets (`nnue_pack`, 100-byte records)

| Set | Where | From | Filter | Train | Val |
|---|---|---|---|---|---|
| `packed_d8` | Mac `corpus_s38/` | pool 1-6 (`corpus_s37c/pool_d8.jsonl`) | none | 355,549,320 | 19,285,765 |
| `packed_half` | Mac `corpus_s38/` | same pool | `--half` (half the training games; val kept) | 177,739,896 | 19,285,765 |
| `packed_quiet` = PC `~/data/ours_quiet` | Mac `corpus_s38/` | same pool | `--quiet` | 222,630,232 | 12,201,870 |
| `packed_s50` | Mac `corpus_hao/`, PC `hao/` | Hao sample 50 | none | 388,975,767 | 8,560,167 |
| `packed_s50q` | PC `/mnt/c/Users/fgant/hao/` | Hao sample 50 | `--quiet` | 229,751,145 | 5,075,521 |
| `hao_q3000` | PC `~/data/` | `packed_s50q` via `mix.py --max-eval 3000` | \|score\| ≤ 3000 | 206,150,012 | 4,559,648 |
| `packed_s150q` | PC `/mnt/c/Users/fgant/hao/` | Hao sample 150 (held out: `thread_index=120`) | `--quiet` | 683,784,419 | 4,134,127 |
| `packed_b1q` = PC `~/data/ours_b1q` | Mac `corpus_s39/` | Mac batch 1 + PC batches 1-2 (178,755,714) | `--quiet` | 109,731,122 | 5,822,174 |
| `packed_b2q` = PC `~/data/ours_b2q` | Mac `corpus_s39/` | Mac batch 2 + PC batch 3 (118,473,086) | `--quiet` | 72,852,017 | 3,863,057 |
| `packed_b4q` = PC `~/data/ours_b4q` | Mac `corpus_s39/` | PC batch 4 (58,669,741) | `--quiet` | 35,864,254 | 1,906,803 |

## 4. Mixes (`trainer/mix.py`, on the PC in `~/data/`)

| Mix | Inputs (train positions) | Train | Val |
|---|---|---|---|
| `mix_q` | ours_quiet 222.6M + s50q 229.8M | 452,381,377 | 17,277,391 |
| `mix_a2` | ours_quiet + s50q + s150q **capped** at 215.5M (Hao 2:1 to ours) | 667,881,377 | 21,411,518 |
| `mix_a1` | ours_quiet + s50q + s150q 683.8M | 1,136,165,796 | 21,411,518 |
| `mix_c1` | ours_quiet + ours_b1q 109.7M + s50q + s150q | 1,245,896,918 | 27,233,692 |
| `mix_c2` | ours_quiet + ours_b1q + ours_b2q 72.9M + ours_b4q 35.9M + s50q + s150q | 1,354,613,189 | 33,003,552 |

## 5. Networks

Trainer **R** = Rust `nnue-offline-trainer`, chained from the previous network; **G** = GPU
`trainer/` from scratch. "h4 recipe" = lr 1e-3, ×0.7 per epoch, outcome weight 0.3 (clamp 2000,
scale 410). S39 runs: 8 epochs of n/4 positions (2 passes) unless noted. Shipped networks in
bold, with the first 12 hex digits of the committed file's sha256.

| Network | Session | Data | Trainer, recipe | Best epoch, val loss | Result |
|---|---|---|---|---|---|
| **s26** (`default-s26.bin`, af092f17d77c) | 26 | 14.0M (S26 teacher corpus + …) | R | unknown | shipped S26 |
| **k81** (`default-k81.bin`, 4128b0956faf) | 35 | batch 1 (62.6M) | R, 2 epochs from the then-default network (which one: unknown; S35 changed it twice), 81 king buckets | pearson 0.813 | shipped S35 (2026-10-17) |
| **p9** (`default-p9.bin`, 13d614617263) | 36 | pool 1-2 (125.0M) | R, 9 king buckets, init unknown | unknown | SPRT vs k81 +100 ± 68, shipped S36 |
| **q9** (`default-q9.bin`, fe14c8fe2e33) | 36 | pool 1-3 (187.2M) | R, epoch 1 of 2 from p9 | unknown | SPRT vs p9 +173 ± 112, shipped 2026-10-21 |
| **r9** (`default-r9.bin`, bf77c37a9650) | 37 | pool 1-4 (249.5M) | R, from q9 | pearson ~0.826 | SPRT vs q9 +34.6 ± 27.9, shipped 2026-10-24 |
| **s9** (`default-s9.bin`, 779d2f7dce36) | 37 | pool 1-5 (312.2M) | R, 1 epoch from r9 | pearson ~0.826 | SPRT vs r9 +80, shipped 2026-10-25 |
| t8 | 37 | pool 1-6 (375M) | R, 1 epoch from s9 | 0.8267 / 0.03737 | vs s9 +27.9 ± 25.8, cap; not shipped |
| t10 | 37 | pool 1-5 + batch 6 at depth 10 | R, 1 epoch from s9 | unknown | vs t8 −18.8, FAIL |
| **g5** (`default-g5.bin`, 7949e4f1fb72) | 38 | `packed_d8` | G, scratch1: lr 1e-3 ×0.95, 60 epochs of 100M | 5, 0.03598 | SPRT vs s9 +93.3 ± 65.2, shipped 2026-10-29 |
| **h4** (`default-h4.bin`, 0a9f9dbe2627) | 38 | `packed_d8` | G, scratch2: h4 recipe, 8 epochs of 100M | 4, 0.03562 | SPRT vs g5 +56.1 ± 44.7, shipped 2026-10-30 |
| sw_w50, sw_w00, sw_g50, deep1, deep2, ft_t8 | 38 | `packed_d8` | G variants (outcome 0.5 / 0; ×0.5; `--mid 32`; fine-tune t8) | (Mac `trainer/runs/`) | all within ~±15 of h4 or FAIL |
| p0_half | 39 | `packed_half` | G, h4 recipe, 8 × 100M | 2, 0.03839 | vs h4 −89, −168 |
| p0_quiet | 39 | `packed_quiet` | G, h4 recipe, 8 × 100M | 5, 0.02873 | SPRT vs h4 +63.5 ± 49.1 (PASS, not shipped: m8 followed) |
| p0_hao | 39 | `packed_s50` | G, h4 recipe, 8 × 100M | 4, 0.04581 | vs h4 −125, −147 |
| p0_hao_quiet | 39 | `packed_s50q` | same | 4, 0.03662 | vs p0_quiet −135, −89 |
| p0_hao_q3000 | 39 | `hao_q3000` | same | 4, 0.03854 | vs p0_quiet −147, −82 |
| **m8** (`default-m8.bin`, af9e6b5a1dbb) | 39 | `mix_q` | G, h4 recipe, 8 × 100M (p1_mix_q) | 8, 0.02988 | SPRT vs h4 +240.8 ± 131.7, shipped 2026-10-31 |
| p1_mix_q16 | 39 | `mix_q` | G, 16 × 100M, ×0.85 | 16, 0.02985 | vs m8 +3, +57 |
| p1_a2 | 39 | `mix_a2` | G, h4 recipe, 2 passes | 8, 0.03047 | vs m8 +64, +61 |
| **a1** (`default.bin`, e842ac27800d) | 39 | `mix_a1` | G, h4 recipe, 2 passes (p1_a1) | 8, 0.02997 | SPRT vs m8 +110.9 ± 76.1, shipped 2026-11-01 |
| p1_c1 | 39 | `mix_c1` | G, h4 recipe, 2 passes | 8, 0.02897 | vs a1 +17 (cap), −13 (FAIL) |
| p1_c1m | 39 | `mix_c1` | same, `--mid 32` (v6) | 8, 0.02900 | vs a1 −7 (FAIL), +20 (cap) |
| p1_c2 | 39 | `mix_c2` | G, h4 recipe, 2 passes | training | screens vs a1 queued |
| p1_c2m | 39 | `mix_c2` | same, `--mid 32` | queued after c2 | |

Weights: Mac `weights/session3x/` (engine `.bin`), run directories in `trainer/runs/` on the
machine that trained them (S39 `p1_*` and `p0_hao*` on the PC, `~/yse/trainer/runs/`).
