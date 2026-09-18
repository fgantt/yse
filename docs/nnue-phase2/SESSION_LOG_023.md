# NNUE Implementation Session Log

## Session 23: Material PSQT Path — NNUE Beats Classical

**Date**: 2026-09-18
**Phase**: Phase 2q
**Objective**: Fix the Session 22 finding that the POV network barely values material, via
material-residual training plus more varied training data, as steps toward a modern NNUE design.

> **READ THIS FIRST.** The network now has a PSQT path (the linear bypass of modern NNUE designs)
> initialised to material values and frozen, so the hidden layers learn only `teacher - material`.
> This network **beats the classical evaluator by +247 ± 91 Elo** over 90 games, the first decisive
> NNUE win over classical in this project. Two more move-generation rules bugs were found and fixed,
> and our legal-move lists now match YaneuraOu's exactly on 6,741 corpus positions.

---

## Headline Results

All bake-offs: `elo-tester`, 90 games, fixed depth 2 (`--depth 2 --fixed-depth`), corrected harness.

| A | B | W-D-L (A) | Elo (A) |
|---|---|---|---|
| POV network (S21) | baseline stm,hand network | 83-2-5 | +458.5 ± 173.4 |
| POV network (S21) | classical | 11-5-74 | **−301.3 ± 101.6** |
| PSQT network (S23) | POV network (S21) | 84-1-5 | +474.6 ± 195.0 |
| PSQT network (S23) | classical | 70-5-15 | **+246.9 ± 90.8** |

The first two matches were run on the Session 22 harness; they confirm that the POV network, although
far ahead of the older network, lost to classical because it threw material away.

Caveat: these four matches ran on binaries built before the two rules fixes below. Both engines in
each match shared the same move generator, so the comparison is fair, but some legal pawn drops were
unavailable and some illegal non-promoting moves were available to both sides.

Held-out evaluation, same recipe and seed as the S21 POV network (`eval_correlation`, every 20th game):

| evaluator | pearson[clamp] | spearman[raw] | slope vs teacher |
|---|---|---|---|
| classical | 0.644 | 0.580 | 0.42 |
| POV network (S21) | 0.641 | 0.595 | — |
| **PSQT network, frozen material** | **0.730** | **0.712** | 0.66 |
| PSQT network, trained (`--psqt-lr 0.5`) | 0.731 | — | — |

Material sensitivity (median eval gain when one opponent piece is removed, cp):

| piece | classical | POV (S21) | PSQT (S23) |
|---|---|---|---|
| Pawn | 130 | 1 | 43 |
| Gold | 499 | — | 569 |
| Bishop | 787 | — | 676 |
| Rook | 988 | 191 | 865 |
| Dragon | 1302 | — | 1407 |

Pawns remain undervalued (43 cp against a PSQT value of 90): the hidden layers learn to partly cancel
the pawn term, presumably because in quiet teacher positions a pawn's value is mostly positional.

Training peaks at **epoch 3** and declines after, against epoch 11 without the PSQT path: the network
now fits the residual quickly and then overfits the 64K-position corpus. More data is the remedy (below).

---

## The PSQT Path (commit `b2b5abd`)

- `NNUEWeights::psqt: Vec<i32>`: one centipawn value per input feature, empty when absent.
- `NNUEAccumulator::psqt`: summed over the active features. Every accumulator update now goes through
  `add_feature` / `sub_feature`, which update hidden layer 1 and the PSQT sum together, so the two
  cannot drift. Make/unmake snapshots save both (`snapshot` / `restore`); the first version saved only
  hidden layer 1 and `nnue_incremental_is_exact_at_every_evaluated_node` caught 718/723 mismatches.
- `evaluate()` returns `network_cp + psqt`.
- `NNUEWeights::material_psqt(rows)`: `+value` for the mover's pieces on board and in hand, `-value` for
  the opponent's, YaneuraOu's material values (`MATERIAL_VALUES`), POV feature space only.
- Trainer: `--psqt-material` initialises it; `--psqt-lr` trains it with Adam (0 = frozen). The forward
  pass reads the f32 shadow so sub-centipawn updates count before they round. The sigmoid scale (410)
  is shared by target and prediction, so the PSQT path and the teacher are on the same cp scale.
- File format v3 (`SHOGINNUE3`): v2 plus the PSQT vector. Written only when a PSQT path exists; v2 and
  v1 files still load. The legacy `save` refuses a network with a PSQT path rather than drop it.

Tests: `material_psqt_scores_material_from_the_movers_side` (oracle against a material count computed
from the board and hands), `material_psqt_survives_save_load_and_the_engine_evaluator`, and the
search-semantics fixture network now carries a PSQT path, so incremental exactness, staleness and
colour symmetry all cover it.

---

## Rules Bugs Found Through the Teacher

### Pawn-drop mate (commit `dc2bf97`)

`is_pawn_drop_mate` checked only king steps: any checking pawn drop the king could not dodge counted as
mate, even when another piece could take the pawn, so those legal drops were never generated. It also
lost a piece when testing a king escape that captures. It now asks whether the defender has any legal
board move after the drop.

Found by the new `corpus_audit`, which checks that each consecutive pair of corpus positions is linked
by one of our legal moves: 4 breaks in 66,903 pairs before, all checking pawn drops, 0 after.

### Forced promotion (commit `566eecf`)

Pawns and lances moving to the last rank, and knights to the last two, were also offered unpromoted.
Found when YaneuraOu rejected a random `6b6a` during corpus generation, which the new move validation in
`corpus-gen` turned into a hard error instead of a silently mislabelled game.

`corpus_audit <corpus> <teacher>` now also compares full legal-move lists with the teacher's `moves`
command on every 10th position: **6,741 positions, 0 moves only we generate, 0 only the teacher
generates.** This is a far stronger oracle than start-position perft, which never reaches drops that
give check or pieces on the last ranks.

The existing corpus was sound: the teacher's labels belong to the recorded positions (re-scoring 120
sampled positions at the same depth differed by a median 71 cp, which is search noise).

---

## More Varied Data (commits `8376f03` and earlier)

`corpus-gen --random-move-prob p` plays a random legal move instead of the teacher's, after the teacher
has labelled the position, sending games into material imbalances that teacher self-play never reaches.
Teacher moves are validated against our move generator; `--game-id-offset` lets parallel runs be
concatenated. The teacher (YaneuraOu V9.00 halfkpvm, `yaneuraou-V900-git-mac-all/`) needs about 1.3 s
per depth-10 game.

Corpus: `corpus_s23/run.sh`, 8 × 1,250 games at random-move rates 0 / 0.05 / 0.10 / 0.15, game ids from
10,000 up; 1,063,022 positions. Combined with the original as `nnue_corpus_s23_combined.jsonl`
(1,130,425 positions; gitignored). `corpus_audit` on it: 0 chain breaks in 1,119,925 pairs and 0
move-list differences from the teacher on 113,043 positions.

All runs: frozen material PSQT, POV + hand, seed 47. Held-out numbers are on the **original** corpus's
held-out games (`eval_correlation`), so they compare with the table above.

| network | variant | pearson[clamp] | spearman | pawn / rook gain (cp) |
|---|---|---|---|---|
| `psqt_frozen` | original 64K corpus | 0.730 | 0.712 | 43 / 865 |
| `big_frozen` | combined corpus | 0.735 | 0.726 | 29 / 649 |
| `big_learned` | + `--psqt-lr 0.5` | 0.733 | 0.727 | 21 / 475 |
| `quiet_lr05` | + `--skip-noisy` | 0.724 | 0.709 | 37 / 541 |
| `quiet_lr01` | + `--skip-noisy`, lr 0.01 | 0.726 | 0.729 | 31 / 463 |
| `ow0_lr05` | + `--outcome-weight 0` | 0.746 | 0.743 | 72 / 588 |
| `ow0_quiet_lr01` | + both, lr 0.01 | 0.718 | 0.715 | 48 / 570 |

The offline metrics barely move and material sensitivity *falls* with the bigger corpus, yet in play:

| A | B | W-D-L (A) | Elo (A) |
|---|---|---|---|
| `big_frozen` | `psqt_frozen` | 74-0-16 | +266.0 ± 99.7 |
| **`big_frozen`** | **classical** | **83-4-3** | **+492.2 ± 179.3** |
| `ow0_lr05` | `big_frozen` | 37-1-52 | −58.5 ± 73.6 |

**Lesson: held-out correlation on teacher-self-play positions does not predict playing strength.** The
original held-out games contain no imbalanced positions, so they cannot see what the random-move data
teaches. Decide between networks by bake-off. `ow0_lr05` has the best correlation and loses (not
significant), so it was not taken further.

`big_frozen` is now `nnue/default.bin` (untracked).

Hypotheses tested for the material compression, neither confirmed: non-quiet training positions
(`--skip-noisy`, commit `8376f03`, default off) and the game-outcome blend (`--outcome-weight 0`).

---

## Next Steps

1. More data along the same line (it was the biggest lever: +266 Elo), and a held-out set that includes
   random-move games, so the offline metric can see what play sees.
2. Toward modern NNUE: train the PSQT path (`--psqt-lr`), then king-conditioned features under POV
   (HalfKP with the king square mirrored), which only becomes worthwhile once the corpus is large enough
   not to overfit.
3. Search speed (depth 3 ≈ 6 s/position) is now the main limit on playing strength and on bake-off cost.

---

## Update — 4.24M-Position Corpus (Session 24 data)

`corpus_s24/run.sh`: 10 × 3,000 more games (random-move rates 0–0.15, seeds 2401–2410, game ids from
100,000 up), 3,109,831 positions, no divergence errors. Combined with everything earlier as
`nnue_corpus_s24_combined.jsonl`: **4,240,256 positions, 40,500 games**. `corpus_audit`: 0 chain breaks
in 4,199,756 pairs, 0 move-list differences from the teacher on 424,026 positions.

**Screening metric.** Held-out games of the S23 combined corpus (which include random-move games) rank
networks much closer to play than the original held-out games did: `psqt_frozen` 0.759 / 0.804
(pearson / spearman), `big_frozen` 0.784 / 0.849, `ow0_lr05` 0.793 / 0.855, classical 0.672 / 0.672.
It separates the +266 Elo pair clearly but ranks `ow0_lr05` slightly above `big_frozen`, which it lost
to (not significant). Use it to screen, the bake-off to decide.

Same recipe as `big_frozen` (frozen material PSQT, POV + hand, outcome weight 0.3, seed 47):

| network | lr | best epoch | S23-combined held-out pearson / spearman | pawn / rook (cp) |
|---|---|---|---|---|
| `s24_lr05` | 0.05 | 3 | 0.796 / 0.864 | 80 / 527 |
| **`s24_lr01`** | 0.01 | 11 | **0.801 / 0.872** | 67 / 452 |

With 4M positions the lower learning rate trains steadily to epoch 11 instead of peaking at once.

| A | B | W-D-L (A) | Elo (A) |
|---|---|---|---|
| **`s24_lr01`** | `big_frozen` | 54-1-35 | **+74.5 ± 74.3** |
| `s24_lr05` | `big_frozen` | 44-1-45 | −3.9 ± 72.4 |
| **`s24_lr01`** | classical | 83-4-3 | +492.2 ± 179.3 |

(The last line equals `big_frozen`'s result against classical by coincidence; the games differ.) Against
classical both networks are near the ceiling a 90-game depth-2 match can resolve, so progress now has to
be measured network against network. `s24_lr01` is now `nnue/default.bin`.

Data scaling so far, each step judged head to head: 64K → 1.13M positions +266 Elo; 1.13M → 4.24M
+74.5 Elo (at a tuned learning rate). The returns are diminishing on this 256→32→1 network, which is
the signal to move to the bigger modern architecture (king-conditioned features, both perspectives),
which needs exactly this kind of data volume.
