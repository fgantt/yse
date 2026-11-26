# Training a network: theory, mechanics and our tools

An illustrated version of this guide is `TRAINING_A_NETWORK.html` beside it (published at
https://claude.ai/artifact/BGPh8Ni2r8yvXmG6KpBwSy).

This guide explains what "training a network" means, how it works inside, and how our tools are used to do it.
Part 1 needs no machine-learning background. Part 2 shows how our network is built. Part 3 is the hands-on part: the
commands, files and checks. Part 4 lists what we have learned, and Part 5 is a glossary.

Source of truth for the numbers below: `docs/nnue-phase2/PLAN_S48.md` (runs h1-h4) and
`docs/nnue-phase2/DATA_LINEAGE.md` (where every data set came from).

---

## Part 1. The idea

### 1.1 What the network is for

A shogi engine does two things over and over:

1. **Search:** look ahead through moves and replies ("if I play this, they play that, then I ...").
2. **Evaluate:** at the end of each line, look at the position and say how good it is, as a single number. Plus is
   good for the side to move, minus is bad.

The search may visit millions of positions per move, and it can only be as good as the scores it gets back. The
**network** is the evaluator: a function that takes a position and returns a score. Before networks, engines used
hand-written rules ("a rook is worth this much, a king with no defenders loses this much"). A network learns its
judgement from examples instead.

### 1.2 A network is a formula with millions of dials

Picture a formula of the form

    score = f(position; w1, w2, w3, ... w32,000,000)

The `w`s are **weights**: plain numbers, like dials on a mixing desk. The formula's shape is fixed (Part 2). Only the
dial settings change. With random settings the network's scores are nonsense. **Training** means finding dial
settings for which the network's scores are good.

Our standard-format network has about 32 million weights. Nearly all of them sit in the first layer, which Part 2
explains. Its file, `nn.bin`, is 64 MB.

### 1.3 Learning from examples

"Good" needs a definition. We use **supervised learning**: we gather a large number of examples, each made of

- a **position**, and
- a **target**: the score we wish the network had given it.

The targets come from a **teacher**: a strong engine searching each position fairly deeply. In our data that is
YaneuraOu at depth 8 to 9. So the network learns to predict, **at a glance, what a deep search would conclude**.
Inside our engine's search, that glance happens at every leaf, so the search behaves as if it saw further than it
does.

We also record each game's **outcome** (who won), and blend it into the target. The teacher's score says "this looks
good". The outcome says "and the side that had it really did win". We use 70% teacher score and 30% outcome
(`--outcome-weight 0.3`).

### 1.4 Measuring "wrong": the loss

For each example, the network gives a score and we compare it with the target. The comparison is not made on the
raw scores. A position judged +3000 that the teacher called +2500 is won either way, so that error should hardly
count. An error between −200 and +200 decides games.

So both numbers first pass through a **sigmoid**, an S-shaped curve that turns a score into something like a win
probability between 0 and 1:

    p = sigmoid(score / 600)        score 0 -> 0.50,  +600 -> 0.73,  +1800 -> 0.95

The error is `(p_network − p_target)²`, averaged over a batch of positions. This is the **loss**: one number that
says how wrong the network currently is. Training means making the loss small. Teacher scores are clamped to ±3000
first, so a few absurd scores cannot dominate (`--clamp 3000`, `--sigmoid-scale 600`).

### 1.5 Turning the dials: gradient descent

With 32 million dials, trial and error is hopeless. But calculus gives, for every dial at once, the answer to "if I
turn this dial up a hair, does the loss go up or down, and how fast?" That set of answers is the **gradient**.
Computing it efficiently through every layer is called **backpropagation**. PyTorch does it for us.

One training **step** is then:

1. Take a **batch** of 16,384 positions.
2. Run the network on them and compute the loss.
3. Compute the gradient.
4. Turn every dial a small amount in the direction that lowers the loss.

The size of that small amount is the **learning rate** (`--lr`). Too big, and the dials overshoot and the loss blows
up. We saw this in the S48 sweep: SGD at rates 3 and 30 collapsed to a constant output. Too small, and nothing is
learned. Each step improves the network on that batch only, and only a little. Millions of steps over billions of
positions add up to a good evaluator.

**Optimizer.** The rule for turning the dials. Plain **SGD** (stochastic gradient descent) moves each dial in
proportion to its gradient. **Adam**, our default, keeps a running average of each dial's recent gradients, and of
how much they vary, and scales each dial's step by them. Dials with consistent signals move steadily, and noisy
ones move cautiously. In our pilots Adam beat SGD clearly (PLAN_S48, "pilots").

**Epochs and the learning-rate schedule.** We cut training into **epochs** of 300 million positions
(`--epoch-size`). An epoch is just a checkpoint interval here, not one full pass over the data. After each epoch, the
learning rate is multiplied by `--gamma` (0.93 in h4). Training thus starts with big steps, to get roughly right
fast, and ends with tiny ones, to settle precisely.

### 1.6 Held-out data: is it learning or memorising?

A network can get good at the exact positions it was shown without getting good at shogi. That is **overfitting**,
like a student who memorises past exam answers. To catch it, a slice of the data, the **validation set**, is never
trained on. After each epoch we measure the loss there (**val loss**), and the correlation between the network's
scores and the teacher's (**val pearson**, 1.0 = perfect). If train loss keeps falling and val loss starts rising,
the network is memorising.

### 1.7 Lower loss is not the same as stronger play

The loss measures agreement with the teacher, position by position. Strength is something else: winning games
inside a search. The two usually move together early on and then part ways. In run h1, val loss kept falling for
6 epochs while strength stopped improving after epoch 3. **So every candidate network is judged by playing games**
(Part 3.6). The loss is used to watch the run and to spot breakage, never to ship.

### 1.8 Why data matters so much

The network can only learn what the examples show. More positions, more varied positions, and better teachers
(deeper search, stronger engine) all help, with diminishing returns:

- Each new kind of position (openings, attacks, endgames, odd material) teaches something.
- Repeats of what is already well covered teach little.
- A teacher's mistakes are learned along with its knowledge.

Strong public networks come from a **loop**: train a network, use it inside an engine to play and label fresh games
at greater depth, train a better network on that, and repeat.

---

## Part 2. How our network is built (the standard format)

We train in the community's standard format, YaneuraOu's **HalfKP 256x2-32-32** (`nn.bin`). That way one file runs
in our engine and in YaneuraOu (`docs/ROADMAP_DUAL_ENGINE.md`). We implement the format with our own code; no
YaneuraOu code is used. "NNUE" stands for *efficiently updatable neural network*. The name comes from the trick in
2.3.

### 2.1 The inputs: what the network "sees"

The network does not see a picture of the board. It sees a list of yes/no **features**. In HalfKP, each feature is
a combination of

    (my king's square, a piece and its square)        e.g. "my king on 5i, their silver on 4c"

with captured pieces in hand counted as "piece in hand, the 1st/2nd/... of that type". There are 81 king squares ×
1,548 piece-and-square cases = **125,388** possible features. In any position about 38 of them are "on" (every
piece except the kings), all others "off".

Tying every piece to the king's square is what lets the network learn king safety: "their gold next to my king"
means something very different from "their gold far away".

The network looks at the position twice, once from each side's king (**two perspectives**). The side to move's
view comes first.

### 2.2 The layers

    125,388 features ──► 256 numbers   (for each side: the "accumulator")     ← 32M weights, nearly all of the net
            2 x 256 = 512 ──► 32 ──► 32 ──► 1 score                           ← ~17k weights

Each arrow is a layer. A layer takes its inputs, multiplies them by weights, adds them up (plus a constant called a
**bias**) and **clips** the result to a range (here 0 to 1, or 0 to 127 in integers). The clipping is what makes it
more than one big weighted sum: without it, all the layers together would collapse into a single linear formula.
The last layer outputs the score.

### 2.3 The trick that makes it fast

Because the inputs are yes/no, the first layer's sum for a position is just *the sum of the weight rows of the ~38
features that are on*. When a move is made, only 2 to 4 features change: the piece leaves one square and lands on
another, and maybe a capture puts a piece in hand. So the engine updates the 256 numbers by subtracting and adding
a few rows, instead of re-adding all 38. Only a king move forces a full recount for that king's side.

The small layers after it are cheap. This is why a network with 32M weights can still evaluate millions of
positions per second. Our engine's loader (`src/evaluation/nnue_std.rs`) converts the file into this incremental
form on load.

### 2.4 Floats in training, integers in the engine

The engine computes with small integers for speed. Training needs smooth real numbers (floats), because the dials
must move by tiny amounts. So the trainer works in floats and **rounds onto the format's integer grid once, at each
epoch's export**. It also keeps the small layers' weights inside their integer ranges throughout training, so for them
export only rounds and never cuts off.

Rounding costs a little accuracy. The trainer reports it as `|int - float|`: the average gap between the rounded
and the float network, in score units. In h4 it is about 18-24 points, with maxima of 145-231. Reducing it, by
training with the rounding simulated ("quantisation-aware training"), is one of our open ideas.

### 2.5 The factoriser

The 125,388 features are many, and a feature such as "my king on 1a, their pawn on 9i" may be rare in the data. To
help rare features learn, the trainer adds a second, shared row per piece-and-square **regardless of king square**,
called the factoriser (`shared` in `trainer/halfkp_model.py`). At export it is folded into every king square's row.
The file format is unchanged.

---

## Part 3. Our tools, step by step

    games ─► labelled positions ─► nnue_pack ─► train.bin / val.bin ─► train.py ─► epoch_NNN.bin (nn.bin)
                                                                              │
                                          checks: std_nnue_check, matches (ourmatch, netmatch), ship gate

### 3.1 Where the data comes from

| Source | What | Teacher | Size |
|---|---|---|---|
| Ours | even-game self-play, every position labelled | YaneuraOu + halfkpvm, depth 8 (`scripts/label_stream.py`) | ~375M; `ours_quiet` 223M after the quiet filter |
| Hao (nodchip, MIT) | public set in YaneuraOu's training format | its publisher's search | s50q + s150q + s200bq, 1.83B |
| Suisho5 depth 9 (nodchip, MIT) | public set, 8.0B records | YaneuraOu + Suisho5, depth 9 | ~4.9B packed so far (`packed_s5`, p1-p4); ~3B more raw on the PC |

Every data set, mix and network is traced in `docs/nnue-phase2/DATA_LINEAGE.md`. Each packed folder carries a
`lineage.json`, and each training run copies it in as `data_lineage.json`.

Licences: we use only data we made or data published under licences that allow it (MIT). No other engine's code,
constants or networks are copied.

### 3.2 Packing: `nnue_pack` (`src/bin/nnue_pack.rs`)

The trainer reads one simple format: fixed **100-byte records** (board squares, pieces in hand, side to move,
teacher score, game outcome). `nnue_pack` converts our data and the public data into it:

```bash
# our labelled corpus (JSONL) -> train.bin (shuffled) + val.bin (every 20th game held out)
cargo run --release --bin nnue_pack -- pack --corpus <corpus.jsonl> --out-dir <packed_dir> [--quiet]

# public sets in YaneuraOu's 40-byte PackedSfenValue format (our own decoder); the last input is held out
cargo run --release --bin nnue_pack -- pack-psv --inputs <f1> <f2> ... --out-dir <packed_dir> [--quiet]

# sanity check a public file: decode a sample and check every stored move is legal
cargo run --release --bin nnue_pack -- psv-check --input <file>
```

`--quiet` drops positions that are in check or whose next move is a capture. A score in the middle of an exchange
is misleading to learn from. On our own data this filter was worth +64 Elo (Session 39).

Building on the PC (Windows), use relative output folders: the Windows program reads `/mnt/c/...` as
`C:\mnt\c\...`.

### 3.3 Feature tables

The trainer must turn a board into exactly the features the engine uses. Rather than re-implementing that in
Python (and risking a mismatch), `nnue_pack` writes the lookup tables **from the engine's own functions**:

```bash
cargo run --release --features yo-diagnostic --bin nnue_pack -- tables --halfkp --out trainer/tables_halfkp.json
```

`trainer/nnue_data.py` reads the records and these tables, and builds each batch's feature lists on the GPU.

### 3.4 Training: `trainer/train.py`

PyTorch. Runs on CUDA (the PC's GPU, ~187k positions/s), Apple MPS or CPU. Setup is in `trainer/README.md`. The h4
run, as an example:

```bash
cd trainer
python train.py --arch halfkp --tables tables_halfkp.json --header-from suisho5_nn.bin \
    --data ours,ours,ours,hao_s50q,hao_s150q,hao_s200bq,packed_s5,packed_s5_p1,packed_s5_p2,packed_s5_p3,packed_s5_p4 \
    --out runs/h4 --epochs 48 --epoch-size 300000000 --lr 1e-3 --gamma 0.93 \
    --sigmoid-scale 600 --clamp 3000 [--resume]
```

| Option | Meaning |
|---|---|
| `--arch halfkp` | train the standard format (the default `engine` is our older a1-style network) |
| `--header-from` | any HalfKP 256x2-32-32 `nn.bin`; only its header (format version, architecture string, hashes) is reused, so engines accept our file. No weights are taken from it |
| `--data a,b,c` | packed folders. Batches are drawn from each in proportion to its size, so listing a folder three times triples its share (our own data, which is small) |
| `--epochs`, `--epoch-size` | 48 checkpoints of 300M positions = 14.4B positions seen, ~27 minutes each on the PC |
| `--lr`, `--gamma` | starting learning rate; multiplied by gamma after every epoch |
| `--optimizer`, `--momentum`, `--plateau N` | SGD instead of Adam; or halve the rate when val loss stalls for N epochs instead of using gamma |
| `--sigmoid-scale`, `--clamp`, `--outcome-weight` | the target and loss of 1.3-1.4 (600, 3000, 0.3) |
| `--resume` | continue from `runs/x/last.pt` after a stop |
| `--warm runs/y/last.pt` | start from another run's network, with a fresh optimizer and schedule (fine-tuning) |
| `--batch`, `--val-n`, `--seed` | 16,384 positions per step; 1M validation positions; random seed |

**Outputs** in `runs/<name>/`:

- `epoch_NNN.bin`: the network after each epoch, a ready-to-use `nn.bin`.
- `last.pt`: the full state, for `--resume`.
- `log.txt`: one line per epoch.
- `data_lineage.json`: where the data came from.

**Reading a log line:**

    epoch  40  train loss 0.02708  val pearson[clamp] +0.9455  val loss 0.02697  lr 5.49e-05  187,350 pos/s
               export nn.bin: |int - float| 23.48 (max 145)

- Train loss: on the batches just trained.
- Val pearson and val loss: on held-out positions (1.6).
- lr: the current learning rate.
- pos/s: speed.
- `|int - float|`: the export's rounding error (2.4).

Losses are comparable only between runs with the same validation folders and the same outcome weight.

### 3.5 Running on the PC

Long runs go on the Windows PC's GPU (in WSL), started from the Mac. The run scripts live in `matches/s48/`, which
is local and not in git:

- `s48_pc_h4.sh` runs `train.py` with `--resume`.
- `hold_h4.sh` keeps an SSH session open and reconnects if it drops. Detached jobs on the PC stall.
- The run stops at a marker file (`~/s48net/stop_h4`) and writes `done_h4` at the end.

Every run gets a new folder; run folders are never reused.

### 3.6 Checking a network

1. **The file is read correctly.** `std_nnue_check` evaluates positions with our engine's fast loader and with an
   independent from-scratch implementation of the format, and requires identical scores, including during a search
   with incremental updates:
   ```bash
   cargo run --release --features yo-diagnostic --bin std_nnue_check -- --net epoch_040.bin --corpus <corpus.jsonl>
   ```
2. **Quick strength checks (games, fixed nodes).** Every 4th epoch, `matches/s48/netmatch/per_epoch_h4.sh` fetches
   the file and plays:
   - `ourmatch.py`: **our engine** with the new network vs our engine with a1 (our shipped network), 10k nodes per
     move, 1 thread, 120 games from fixed openings.
   - `netmatch.py`: **YaneuraOu** with the new network vs YaneuraOu with Suisho5's, 100k nodes, 120 games.

   Fixed node counts make the results independent of how busy the machine is. 120 games give about ±60 Elo
   (95%), so only large differences show up. Picking a final epoch takes a few hundred games.
3. **The ship gate.** Before a network replaces a1 in the engine: a two-seed SPRT [0, 40] at 1 s per move with
   4 threads, as real games are played. An SPRT is a match that stops as soon as the evidence is clear. It needs
   the user's go-ahead.

**Reading match numbers:** 50% = equal. 55% ≈ +35 Elo, 64% ≈ +100, 76% ≈ +200, 80% ≈ +240 (8 wins in 10).

### 3.7 Using a network

- **Our engine:** USI option `NNUEFile` = path to the `nn.bin`. Standard-format files are detected and converted on
  load. `NNUEScale` (percent, default 100) scales its scores; at 80 and 60 it did not help h2.
- **YaneuraOu** (and other engines that read the format): put the file in a folder as `nn.bin` and set `EvalDir` to
  it.

---

## Part 4. What we have learned so far (S48)

| Run | Data | vs a1, our engine | vs Suisho5, YaneuraOu |
|---|---|---|---|
| h1 | Hao only, 1.8B | −200 to −240 | ~−260, flat after 3 epochs |
| h2 | ours + Hao + Suisho5 part | up to 44% | −73 to −144 (epochs 8-16) |
| h3 | h2 fine-tuned at a low rate | flat; stopped | flat |
| pilots | outcome weight 0.5; SGD | both worse than the baseline | both worse |
| h4 | ours + Hao + ~4.9B Suisho5, 48 epochs | 45-54% (epochs 24-40) | −50 to −67 (epochs 32-40) |

Lessons:

- **Our own data matters.** h1 without it stalled far below a1. Mixing it in was the first big gain.
- **Val loss keeps falling after strength stops rising.** Choose networks by games.
- **The recipe is near its limit for this network size.** A published reference result, this format trained on
  8B Suisho5 positions, reached −56 against Suisho5. h4 is at about −60.
- **Inside our engine, a network alone gives limited gains.** a1 is level with YaneuraOu's older halfkpvm network
  in our engine (+3 ± 33), and Suisho5's own network beat halfkpvm there by only +35 at 10k nodes. The ceiling
  check queued in PLAN_S48 measures this directly (Suisho5's network and h4 vs a1 at 10k, 100k and 1M nodes).
  Network gains grow with search depth, which 10k nodes understates.
- **Next levers for the network:**
  - our own data generated with the stronger network at greater depth (the loop of 1.8);
  - training that simulates the rounding (2.4);
  - the remaining 3B Suisho5 positions;
  - possibly a larger standard layout.

  For playing strength in our engine, the search is the bigger lever (`docs/ROADMAP_DUAL_ENGINE.md`, goal 5).

---

## Part 5. Glossary

| Term | Meaning |
|---|---|
| accumulator | the 256 first-layer sums per side, updated incrementally as moves are made |
| Adam / SGD | rules for turning the weights each step (1.5) |
| batch | the positions used for one training step (16,384) |
| bias | a constant added in a layer, learned like a weight |
| clipping | limiting a layer's outputs to a range (0-1 in float, 0-127 in integers) |
| epoch | here, a checkpoint every 300M positions: validate, export, save |
| export | rounding the float network onto the format's integers and writing `nn.bin` |
| feature | a yes/no input: (king square, piece, square) in HalfKP |
| gradient | for each weight, which way and how fast the loss changes if it moves |
| held-out / validation set | positions never trained on, used to check for memorising |
| learning rate | how far the weights move per step |
| loss | the average error on win-probability scale; training minimises it |
| NNUE | efficiently updatable neural network: the design of 2.3 |
| overfitting | learning the training examples rather than the game |
| pearson | correlation of network scores with teacher scores (1.0 = identical ranking and spacing) |
| quiet filter | dropping positions in check or mid-capture before training |
| sigmoid | the S-curve mapping a score to a win-probability-like number |
| SPRT | sequential probability ratio test: a match that stops when the result is clear |
| teacher | the deep search whose scores the network learns to predict |
| weights | the adjustable numbers of the network |
