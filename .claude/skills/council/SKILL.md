---
name: council
description: Convene four models from different providers to advise on Yggdrasil engine work - what to try next for strength or speed, a plan before a long run, a code review before a gate or ship, or reading a result. Trigger when the user says "council".
---

# Council (Yggdrasil)

Adapted from Theo Browne's council skill (github.com/t3dotgg/tstack, `skills/council`) for this
engine. Four strong models from four providers review the same question independently, and
the lead (you) weighs what they say. They advise; you decide, and you keep the scope small.

## Members

Unless the user names members, use the latest flagship model of each of four providers at
high reasoning effort. Each time, look the models up with `orchestrator_capabilities` (and
`search: "gemini"` and similar if one is missing from the short list). As of 2026-11-08 these are:

| Member | `providerInstanceId` | `model` | options |
|---|---|---|---|
| Claude Opus | `claudeAgent` | `claude-opus-5-5` | `effort: high` |
| GPT Sol | `codex` | `gpt-6.1-sol` | `reasoningEffort: high` |
| Gemini | `cursor` | `gemini-4-argon` | `reasoning_effort: high` (its maximum) |
| Muse | `muse` | `muse-wonder-1.0` | `reasoningEffort: high` |

If you cannot reach all four providers, or a named member is unavailable, tell the user. Do not
substitute another model.

## Before convening

- Fix the issues you already know about, address open PR comments and run the relevant cheap
  checks (unit tests, `search_bench --fingerprint` when a change should keep the search
  identical). The council reviews the current state, not a version you already plan to change.
  Skip this for code the user wants to throw away.
- Find the engine checkout. The NNUE engine is on `main`, normally in the
  `yse-worktrees/nnue` worktree. The repo root `yse/` is usually on `legacy-main`, the old
  classical engine. Check with `git worktree list`. Delegates start in the parent's checkout
  (`isolation: shared`), so give them the absolute path of the engine checkout. Never use
  `isolation: worktree`: it branches from the parent's commit, which may be `legacy-main`, and
  leaves out uncommitted work.
- Pick the mode (below) and write one brief that all four members get unchanged.

## Modes

1. **Direction**: what to work on next for playing strength or speed.
2. **Plan check**: a proposed run of hours (SPRT, training, data download or labelling, SPSA)
   before the user is asked for machine time. When you are about to ask for such a run, offer a
   council in one line. Do not convene one unasked.
3. **Code review**: a diff before it is gated or shipped. Each member gives a score out of 10.
4. **Result reading**: a gate, screen or diagnostic that has just finished. Is the conclusion
   drawn from it supported?

## The brief

Each member starts cold and has none of this conversation. The Codex, Cursor and Muse members
cannot see Claude's auto-memory either. Put everything they need in the task text:

- **The request and the decision it feeds**, in the user's words where possible.
- **Paths, absolute:** the engine checkout; the current research doc
  (`docs/nnue-phase2/RESEARCH_S<N>.md`, highest N) and the latest `SESSION_LOG_<N>.md`, naming
  the sections that matter; for a review, the exact diff command (`git -C <checkout> diff
  <base>..<head>`, or `git diff` for uncommitted work) and the files touched; any match logs or
  `matches/user_games/*` files involved.
- **The measured facts that frame the question.** Copy the numbers with their source (doc and
  section). Do not paraphrase them into conclusions. Include the numbers that cut against your
  own view.
- **The project rules block below, verbatim.**
- **What to return** for the mode (below), and a limit: about 800 words, with `file:line` and
  doc section references.

### Project rules block (paste verbatim)

```text
Ground rules for this review of the Yggdrasil shogi engine (Rust, USI, NNUE):

Read-only.
- Do not edit, create or delete files. Do not commit, check out, stash or change git state.
- Do not start engines, matches, SPRTs, training, downloads or long scripts. The machine is shared
  with the user's own games and with running jobs, and load ruins match results. If a number would
  settle a question, say exactly what to measure, how, and how long it takes. The lead runs it.
- Compiling is not needed for a review. If you must, run `cargo check` only, with
  CARGO_TARGET_DIR set to a temporary directory and
  SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX.sdk
  (without it the build fails with a tapi/arm64e.x1 error). Never write to the checkout's
  target/, because the user's GUI runs that binary. Never run cargo fmt: the crate is not
  rustfmt-clean and it rewrites dozens of files.

Goal and constraints.
- The goal is general playing strength: even games against YaneuraOu-class engines at
  1 s-per-move-and-up time controls. Do not special-case handicap games.
- YaneuraOu (GPL) may be studied for ideas only. Never propose copying its code, tuned constants,
  formulas or networks. Its network is a diagnostic only (feature `yo-diagnostic`). Published,
  permissively licensed training sets (for example nodchip's MIT Hao/Suisho5 depth-9 sets) may
  be used.
- The engine plays with 4 threads (Lazy SMP), 256 MB hash and MoveOverhead 100. Anything that
  keeps state across moves or threads must be checked as games use it. Helper threads are spawned
  each move (src/search/lazy_smp.rs), and their history and correction tables survive only through
  park/unpark. The TT ages once per move. Bugs of this kind (16 MB hash, cold helper history
  tables) have been invisible to fresh-process benchmarks and were worth +40 to +200 Elo.

How results are measured here (read before estimating anything).
- The ship gate is a pooled two-seed SPRT [0, 40] at 1 s byoyomi, 4 threads per engine. A true
  +20 Elo passes about 39% of the time (median ~510 games), a true +40 about 90% (~306 games).
  A change worth +20 or less cannot be shown on its own. Stack it or don't test it.
- Screens are four parallel SPRTs at 250 ms. After MoveOverhead that is only about 150 ms of
  search. SPSA has run at about 80 ms on 1 thread. Search-pruning gains found there have twice
  disappeared at gate conditions.
- USI "nodes"/nps count main-search nodes only (about 25% of the tree; quiescence is excluded).
  search_bench all_nodes counts everything. Fixed-depth search_bench fingerprints can change
  because of a speedup alone, since IID reads the clock.
- A fixed-node judge has a horizon. Confirm a "blunder" in a sharp position with a deeper search.
- For any ratio, say what the denominator contains. Before concluding the sign of an effect, vary
  the parameter (depth, size, positions). Many confident diagnoses here have been wrong for
  exactly these reasons.

What a proposal must contain.
- The mechanism, and the evidence from this engine's own measurements (cite doc and section).
- A prior from the field, with its source and how well it transfers (search conditions, data
  size, network size).
- The expected Elo as a range, and the chance of passing the gate.
- The cheapest offline check (minutes, no games) that could kill it, and what result would.
- Machine cost: hours of the Mac (user's machine, blocks their games) and of the Windows PC
  (GPU training, labelling).
- For anything that takes hours, answer all four: (1) expected effect against what the gate
  resolves; (2) correctness and state lifetime as 4-thread games use it; (3) do the test
  conditions match the gate; (4) does it address the measured gap?
```

### What to ask for, by mode

- **Direction.** At most three candidates, ranked. Each comes with the proposal contents above.
  Also ask: which of our current beliefs (in the research doc) is most likely wrong, and what
  would show it? Ask them to look across the field (Stockfish lineage, YaneuraOu and other shogi
  engines, published NNUE, search and training write-ups) and to say where they are guessing.
- **Plan check.** Their own answers to the four questions, their pass odds, a cheaper test that
  should come first if there is one, and the result that should stop the run early.
- **Code review.** Correctness first: thread and state lifetime, TT use, time management, USI
  handling, move legality. Then whether `search_bench --fingerprint` should stay identical, and
  why; performance risk on the hot path; missing tests (a new test must fail without the fix).
  Score out of 10, with what would raise it.
- **Result reading.** Does the data support the conclusion? What does each ratio's denominator
  contain? Other explanations (machine load, test conditions, judge horizon, sample size), and
  what follows for the plan.

## Running it

Start the four with `delegate_task` in one turn, in parallel: `role: "review"` (or
`"research"` for direction), `interactionMode: "plan"` (Muse rejects plan mode: give it
`"default"` and rely on the read-only rules in the brief), `isolation: "shared"`, mode `async`,
and a title like `Council: <topic> (<member>)`. Follow each task's `next`. Do not poll. If a
member fails or times out, say so in the summary and do not fill in its view.

## Summarise

Report to the user:
- **Where they agree**: the findings and recommendations to act on.
- **Where they disagree**, and your judgement with the reason. A member's confidence is not
  evidence. Check its claims against the code and the docs before repeating them, and say which
  ones you checked.
- **Claims that did not hold up.** Name them, so they don't come back later as fact.
- **Recommended next step**, with machine cost and pass odds. Anything of hours still needs the
  user's go-ahead.

Record what the decision changed in the current research doc as a short "Council (date, mode)"
note, with the members and models, the points adopted and the points rejected. Their full
answers stay in their threads; link them.

## Review rounds and scores

For code review, each full pass by all four members is one round. A fresh review after fixes
starts a new round, with a new brief that lists what changed since the last round and the
objections that were not adopted, with your reasons. If you agree with a recommendation, make the
change, then ask for a fresh review. Continue until all four are satisfied, or until what remains
is out of scope. Fight scope creep: a review that turns into a feature list gets cut back to the
change under review, and good ideas outside it go into the research doc as candidates, not into
this change.

If the work ends in a pull request, put this section in the PR description, before the model and
harness line at the end (edit the description; do not post a comment):

```markdown
## Council review

Rounds: 2

| Delegate | Round 1 | Round 2 |
| --- | --- | --- |
| Claude Opus 5.5 | 7/10 | 9/10 |
| GPT-6.1 Sol | 8/10 | 10/10 |
| Gemini 4 Argon | 7/10 | 9/10 |
| Muse Wonder 1.0 | 8/10 | 9/10 |
```

If a member gives no score, write `no score`. Never invent one. With no PR, put the same table
in the final reply.
