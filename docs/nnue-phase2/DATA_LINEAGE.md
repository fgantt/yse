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
| `corpus_s39/b1_s5`, `pc1_s5`, `pc2_s5` | S39 | the games of Mac batch 1, PC batches 1-2, same ids | **relabelled by Suisho5** at depth 8 (`FV_SCALE` 24; `LABEL_OPTIONS`, `YSE_YANE`); pc1 on the PC (`~/relabel/`), the others on the Mac | in progress (2026-11-02) |

Batches 1-6 were deduplicated as they were pooled (first record of a repeated position kept,
`corpus_s36/dedupe_pool.py`), into cumulative pools: `corpus_s36/pool_d8.jsonl` (1-2, 125.0M),
`corpus_s36b/pool_d8.jsonl` (1-3, 187.2M), `corpus_s37/pool_d8.jsonl` (1-4, 249.5M),
`corpus_s37b/pool_d8.jsonl` (1-5, 312.2M), `corpus_s37c/pool_d8.jsonl` (1-6, 374,835,085) and
`pool_d10.jsonl` (1-5 at depth 8 + batch 6 at depth 10). **Stage B (S39) batches were not
deduplicated**, only concatenated. PC games are generated on the PC and labelled on the Mac.

Stage B measured against batches 1-6 (2026-11-02, samples of 15-30k games per source): game
length (~115 plies), colour balance, score spread, the share of positions whose depth-8 score
agrees with the result, and opening variety (distinct positions at ply 16) are the same for
Mac B1, the PC's Suisho5 games and batches 1 and 6; the m8/a1-vs-YaneuraOu games differ only
slightly. About 6% of each batch repeats within it and a few percent repeat earlier batches,
mostly opening positions; because Stage B was not deduplicated, its packed sets keep them
(median |score| 526 quiet, against 703 for the deduplicated pool). Stage B is the same
distribution and the same teacher as batches 1-6, whose last batch already added little (t8).

**Suisho5 labels' scale.** On 858k positions of Mac batch 1, Suisho5 at depth 8 (FV_SCALE 24)
correlates 0.97 with halfkpvm but its scores are 0.78 as large (median |score| ratio 0.78;
the logistic scale that best predicts the game result is 560 against halfkpvm's 720). Hao's
scores match halfkpvm's, so Suisho5 labels are packed with `--eval-scale 1.29`.

Before Session 35: corpora `corpus_s23` … `corpus_s28`, `corpus_self`, `corpus_hc`,
`corpus_rank` fed networks up to s26 (14.0M positions, Session 26); see those session logs.

## 2. Published data

| Set | Source | Files | Positions |
|---|---|---|---|
| Hao sample 50 | nodchip `shogi_hao_depth9` (Hugging Face, MIT), depth-9 labels by its publisher | `docs/nnue-phase2/hao/sample50.txt` (name, size, sha256; spread over the set's 8 generation runs) | 397.5M |
| Hao sample 150 | same set | `docs/nnue-phase2/hao/sample150.txt` (all 150 packed; 2 were already on disk) | 1,166M |
| Hao sample 200b | same set | `docs/nnue-phase2/hao/sample200b.txt` (25 per generation run, none in samples 50/150; 62.7 GB, fetched 2026-11-03) | 1,567M |
| Suisho5 depth 9 | nodchip `shogi_suisho5_depth9` (Hugging Face, MIT), depth-9 labels by Suisho5, already shuffled and replaced by quiescence leaves (no stored move) | all 8 volumes `shuffled.7z.001`-`.008` (184.6 GB, sha256 from the Hugging Face listing, fetched 2026-11-08) + `shuffled.bin` (290 MB, 7,240,673, validation; Mac `corpus_suisho5/val/`) | ~5B |

Raw files: Mac `corpus_hao/raw/` (sample 50), PC `/mnt/c/Users/fgant/hao/raw/` (samples 50, 150, 200b). Suisho5 volumes: PC `/mnt/c/Users/fgant/suisho5/raw/`.

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
| `packed_s200bq` | PC `/mnt/c/Users/fgant/hao/` | Hao sample 200b (held out: its last file) | `--quiet` | 920,303,519 | 4,297,099 |
| `packed_b1q` = PC `~/data/ours_b1q` | Mac `corpus_s39/` | Mac batch 1 + PC batches 1-2 (178,755,714) | `--quiet` | 109,731,122 | 5,822,174 |
| `packed_b2q` = PC `~/data/ours_b2q` | Mac `corpus_s39/` | Mac batch 2 + PC batch 3 (118,473,086) | `--quiet` | 72,852,017 | 3,863,057 |
| `packed_b4q` = PC `~/data/ours_b4q` | Mac `corpus_s39/` | PC batch 4 (58,669,741) | `--quiet` | 35,864,254 | 1,906,803 |
| `packed_b1s5q` = PC `~/data/ours_b1s5q` | Mac `corpus_s39/` | `b1_s5` + `pc1_s5` + `pc2_s5` | `--quiet --eval-scale 1.29` | 109,731,122 | 5,822,174 |
| `packed_s5` | PC `/mnt/c/Users/fgant/suisho5/` | first 922.7M records of Suisho5 depth 9 (a1's Hao counts); held out: the last 9,209,648 | none (already quiet; `--quiet` would drop all, no stored move); no `--eval-scale` (logistic scale 630 vs Hao 590) | 913,535,564 | 9,209,648 |

## 4. Mixes (`trainer/mix.py`, on the PC in `~/data/`)

| Mix | Inputs (train positions) | Train | Val |
|---|---|---|---|
| `mix_q` | ours_quiet 222.6M + s50q 229.8M | 452,381,377 | 17,277,391 |
| `mix_a2` | ours_quiet + s50q + s150q **capped** at 215.5M (Hao 2:1 to ours) | 667,881,377 | 21,411,518 |
| `mix_a1` | ours_quiet + s50q + s150q 683.8M | 1,136,165,796 | 21,411,518 |
| `mix_c1` | ours_quiet + ours_b1q 109.7M + s50q + s150q | 1,245,896,918 | 27,233,692 |
| `mix_c2` | ours_quiet + ours_b1q + ours_b2q 72.9M + ours_b4q 35.9M + s50q + s150q | 1,354,613,189 | 33,003,552 |
| `mix_c1s` | as `mix_c1` with ours_b1s5q in place of ours_b1q (teacher test) | 1,245,896,918 | 27,233,692 |
| `mix_h2` | ours_quiet + s50q + s150q + s200bq (more Hao) | 2,056,469,315 | 25,708,617 |
| `mix_h3` | as `mix_h2` with ours_quiet listed **twice** (ours ≈ 19.5%, a1's share); val holds ours' val twice (S40). **Deleted 2026-11-08** for disk (rebuild: same `mix.py` inputs; lineage in Mac `weights/session40/lineage/`) | 2,279,099,547 | 37,910,487 |
| `mix_s5` | ours_quiet + packed_s5 (`mix_a1` with Suisho5 in place of Hao; N4) | 1,136,165,796 | 21,411,518 |

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
| p1_c2 | 39 | `mix_c2` | G, h4 recipe, 2 passes | 8, 0.02866 | vs a1 −33 ± 54 (FAIL), −91 ± 83 (FAIL); not shipped |
| p1_c2m | 39 | `mix_c2` | same, `--mid 32` (v6) | 8, 0.02856 | vs a1 0 ± 32 over 452 games (FAIL), −74 ± 75 (FAIL); not shipped |
| p1_c1s | 39 | `mix_c1s` | as p1_c1 | 8, 0.02913 | vs p1_c1 −28 ± 51 (FAIL), −14 ± 43 (FAIL); vs a1 +3 ± 30 over 514 games (FAIL), −56 ± 64 (FAIL); not shipped |
| p1_h2 | 39 | `mix_h2` | as p1_a1 | 8, 0.02992 | vs a1 +25 ± 27 (cap, 600 games), +35 ± 27 (PASS); ship SPRT vs a1 at 1 s +20.9 ± 25.4 over 600 games (310-16-274), INCONCLUSIVE at the cap (LLR +0.22); not shipped |
| p1_h3 | 40 | `mix_h3` | as p1_a1 (`938ebeb573e1`) | 8, 0.02890 | vs a1 +40.8 ± 33.4 (PASS), +53.4 ± 42.8 (PASS); vs p1_h2 −137.0 ± 107.9 (FAIL, 40 games), +9.8 ± 27.1 (600, inconclusive); pooled 317-8-315, LLR −4.21 FAIL: no better than h2, not shipped |
| p1_s5 | 40 | `mix_s5` | as p1_a1 (N4: Suisho5 labels in place of Hao) | 8, 0.02892 (own val set; not comparable) | vs a1 on the PC, 4dfe9c4 search, 250 ms: +4.7 ± 27.8 (FAIL, 586), −17.8 ± 44.4 (FAIL, 234); pooled 398-20-402, LLR −6.02 FAIL: no better than a1, not shipped |

Stage B outcome (2026-11-02): adding the S39 batches (`ours_b1q`, `ours_b2q`, `ours_b4q`) to a1's
data lowered held-out loss on each mix's own validation set but did not win games: c1/c1m about
+7/+11 pooled, c2/c2m below a1. The middle layer made no measurable difference.

Teacher test (2026-11-03): relabelling c1's Stage B positions with Suisho5 at depth 8 (p1_c1s)
did not beat the halfkpvm labels (vs c1 −28 and −14, both FAIL) or a1. At depth 8 on our own
self-play positions the choice between these two teachers is not the lever.

Weights: Mac `weights/session3x/` (engine `.bin`), run directories in `trainer/runs/` on the
machine that trained them (S39 `p1_*` and `p0_hao*` on the PC, `~/yse/trainer/runs/`).
