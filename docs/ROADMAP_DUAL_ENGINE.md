# Roadmap: two engines, shared networks (decided 2026-11-24)

The user chose a dual route after the S47 council on routes to parity (`docs/nnue-phase2/PLAN_S47.md` §10 on branch
`s47-core`). Read this before planning engine work in a new session.

## Goals (user, 2026-11-24)

1. Train our standard-format network to surpass a1.
2. Our standard-format network can be used with other engines.
3. Our standard-format network can be used with our engine.
4. Other standard-format networks can be used with our engine.
5. Develop our new search core, using our and others' standard-format networks.
6. Repeat on the other side of the dual path: the GPL engine with each network, measured as a matrix of engine x network.

Standing rule behind them: where a community standard exists (USI, `nn.bin`, SFEN, KIF/CSA, book and training-data
formats), adopt it with our own code; a custom format needs a measured reason and an export to the standard.

| Goal | Status (2026-11-24) |
|---|---|
| 1 | run h1 (PLAN_S48, branch `s48-net`): epoch 4 is ~−250 vs Suisho5 in YaneuraOu; not yet measured against a1 directly |
| 2 | done for YaneuraOu: it loads our `nn.bin` and searches normally |
| 3, 4 | works only in the `yo-diagnostic` build (from-scratch evaluation, slow); Suisho5's file plays correctly in it. Needs a first-class loader (incremental, SIMD with runtime dispatch, an option in the normal build) and a licence check per third-party network |
| 5 | not started; design in PLAN_S46 §4 and PLAN_S47 §9 |
| 6 | not started |

## The three projects

| Project | What | License | Status |
|---|---|---|---|
| **Yggdrasil engine** (this repo, `main`) | our own Rust engine, independent search; loads our networks and other people's networks in the standard format (MIT or otherwise licensed for that use) | user's choice; **no license file yet** | shipped 17a2e37; search ~3 doublings behind YaneuraOu at equal nodes (PLAN_S47 §1, §5) |
| **GPL engine with our networks** | YaneuraOu (GPLv3), unmodified at first, running our networks in its standard format; a fork only if its code ever needs changing | GPLv3 for the engine; our networks stay ours | not started; first step below |
| **GUI** (`ysu`, separate repo) | runs either engine over USI | user's choice; **no license file yet** | next focus, before deep engine work |

## Rules that keep the projects separate

- **No GPL code enters this repo.** YaneuraOu and Stockfish are studied for ideas only, as before. Work on this engine's
  search never reads GPL source. If a GPL fork is ever started, it lives in its own repo, and code never moves from it
  into this one.
- **Networks are the shared asset.** Train in the standard format (YaneuraOu's HalfKP 256x2-32-32, `nn.bin`), so one file
  runs in both engines. Networks we train from our own data and from MIT data (nodchip's Hao and Suisho5 depth-9 sets)
  are ours to license. Other people's networks carry their own terms: check each before shipping or bundling one.
- **Bundling:** the GUI may ship the GPL engine as a separate program it talks to over USI, with the engine's source
  offered as GPLv3 requires.
- **Measure search with the same network in both engines:** then the gap between engines is search alone.

## Order of work (as the user set it, 2026-11-24)

1. **Now: a standard-format network.** Our trainer (`trainer/`) learns a HalfKP 256x2-32-32 network and exports `nn.bin`;
   checked against our loader (`src/evaluation/yo_nnue.rs`), then in YaneuraOu against Suisho5 at fixed nodes on the PC
   (a copy of the engine in a new folder; never the user's GUI engine folder). Quick win: a strong GPL engine running our
   network.
2. **Next: the GUI.** Polish `ysu` and lay out how it presents the engines and networks. Pick licenses for the engine and
   the GUI before going public or inviting contributors (no license file means nobody else may legally use or contribute).
3. **Then: deep two-engine work.**
   - This engine: the search rewrite (PLAN_S46 §4, PLAN_S47 §9), judged against YaneuraOu running the same network, with
     the kill rule of PLAN_S47 §9. Design choices are judged inside the new search after tuning, never by tests on lean.
     Then speed (allocation-free search, staged generation, SEE, SIMD network with runtime dispatch). A first-class
     loader for standard-format networks (incremental accumulator, not the diagnostic's from-scratch evaluation).
   - GPL side: stronger networks (more data, our own self-play), measured in YaneuraOu against Suisho5.
4. **Contributors** once the GUI is presentable and the licenses are set.

## Expectations (council, 2026-11-24; guesses, not measurements)

- This engine alone: +160 to +450 Elo over 4-6 months; parity with YaneuraOu at equal hardware unlikely within 6 months
  (Sol 5-15%, Grok 25-35%).
- YaneuraOu with our network: as strong as our network is; parity with YaneuraOu + Suisho5 depends on matching Suisho5's
  network (+41 Elo over halfkpvm at 1M nodes, growing with depth).
