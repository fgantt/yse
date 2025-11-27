ENGINE GAMEPLAY STABILITY PLAN
================================

Overview
--------
Two recent engine-versus-engine losses (Game A: passive bishop shuffles that allowed △8七歩成; Game B: king-first improvisation culminating in ▲4四角??) exposed systemic weaknesses in Yggdrasil’s early-game heuristics and defensive reactions. Both games collapsed before move 20 without complex tactics. This document consolidates the required corrective actions.

Objectives
----------
1. Enforce disciplined opening templates so the engine never “passes” the first 10 plies.
2. Guarantee timely castle formation and coherent king safety scoring.
3. Prevent redundant major-piece moves and self-destructive captures.
4. Detect and react to rook-file pawn storms (8th file, central files) before they become decisive.
5. Improve offensive coordination so initiative converts into concrete attacks instead of drifting pieces.
6. Provide a validation loop (self-play + targeted benches) to ensure new heuristics improve outcomes without regressing performance.

Root Causes Recap
-----------------
### Game A: △8七歩成 crush
- Repeated bishop oscillation (▲7九角→▲8八角→▲7九角) bled three tempi.
- No castle nor 7筋/8筋 pawn cover, leaving king on 5九 then 5八 under attack.
- Failure to contest △8四–△8五–△8六 led to a forced pawn promotion on 8七 and immediate loss.

### Game B: ▲4四角?? blunder
- Move 1 king advance (▲4八玉) and move 3 rook swing (▲3八飛) produced disharmony reminiscent of a broken Ureshino setup.
- Bishop pushed to 5五 then voluntarily traded on 4四 for a pawn, surrendering initiative and material.
- King never entered any castle shell; Gote’s △6二角 drop created an unstoppable horse/major piece assault.

Strategic Mitigations
---------------------
### 1. Opening Template Enforcement
- Populate the opening book and heuristic priors with vetted structures (static rook, ranging rook, disciplined Ureshino) ensuring early pawn pushes (`▲7六歩`, `▲2六歩`) precede king/rook moves.
- Hard-block first-ply king moves and aimless major-piece moves via evaluation penalties unless a tactical PV proves ≥ +150 centipawns.
- Track “opening debt” (count of non-developing moves) and penalize exceeding a configurable threshold before move 12.

### 2. Castle Formation Requirements
- Define minimum castle targets (e.g., simple gold castle: `▲6八玉`, `▲7八玉`, `▲6八金`) and award increasing penalties per ply when unmet once opponent starts edge or central pawn storms.
- Integrate castle progression into evaluation tables so the king receives credit only when protected by two surrounding gold/silver pieces and a pawn shield.

### 3. Bishop & Rook Coordination Rules
- Add a redundant-move tax: consecutive moves of the same piece without capture or threat incur escalating penalties.
- Require static-rook alignment: discourage `▲3八飛` unless `▲2六歩` is played and `▲2五歩` is plausible within the next two plies.
- Introduce SEE (Static Exchange Evaluation) guardrails before allowing voluntary trades such as `▲4四角??`; reject trades losing more than 100 centipawns absent compensating threats.

### 4. Pawn-Storm Detection & Response
- Detect opponent pawn chains reaching `8六`, `6六`, or analogous squares while our castle is incomplete; prioritize blocking moves (`▲8六歩`, `▲7八金`, drops) or immediate counterplay.
- Score storms dynamically: add penalties to evaluation proportional to (a) number of consecutive opposing pawn advances on a file, and (b) time since last defensive response.
- Expand drop heuristics to consider pawn drops (`▲8八歩打`) or gold placements when storms are imminent.

### 5. Search & Move-Ordering Adjustments
- Increase move-order bonuses for castle-progressing moves and defensive resources against detected storms.
- Bias iterative deepening toward lines that complete development before move 12; reduce search effort on king-first or bishop-shuffle continuations unless they already show clear tactical wins.
- Add blunder filters in quiescence to revisit positions where we consider self-destructive trades or ignore promoted pawn threats.

### 6. Offensive Pressure Framework
- Track initiative metrics (space advantage, piece activity, open files) and inject bonuses when coordinated attacks on the king or weak squares are available, nudging search toward proactive plans rather than passive shuffling.
- Expand pattern-recognition in the evaluation layer to reward classic attacking structures (e.g., climbing silver, edge attacks, rook file openings) once development milestones are met.
- Add move-order incentives for launching synchronized pawn breaks (such as `▲2五歩`, `▲5五歩`) when our pieces already aim at the targeted sector, ensuring offense triggers only when preparation exists.
- Incorporate “attack debt” scoring: if the engine accumulates clear attacking resources but fails to convert them within a set ply window, apply a small penalty to encourage timely execution and avoid giving the opponent counterplay.

Validation & Testing
--------------------
1. **Self-Play Suites**: Create FEN snapshots right before △8七歩成 and ▲4四角??; ensure new heuristics choose protective alternatives.
2. **Opening Regression**: Run 200 self-play games per canonical opening (static rook, ranging rook, Ureshino) to confirm the engine completes castles and avoids king-first improvisations.
3. **Benchmarks**: Execute existing SIMD benches (`benches/simd_performance_benchmarks.rs`, `benches/simd_nps_benchmarks.rs`) to detect runtime regressions after heuristic changes.
4. **Eval Consistency**: Add unit tests for evaluation components verifying penalties (bishop redundancy, storm alerts) and castle bonuses.

Implementation Roadmap
----------------------
1. **Evaluation Layer Updates**
   - Add new terms for castle progress, redundant piece moves, storm detection, offensive coordination, and SEE safeguards in `src/moves.rs` and related evaluation modules.
2. **Opening Policy & Book**
   - Update `docs/design/opening-book/` tasks to include mandatory anti-king-first rules; regenerate the opening book with the new templates.
3. **Search Integration**
   - Modify move ordering and pruning heuristics (e.g., `search-algorithm-optimizations`) to respect the new development, offensive, and defense priorities.
4. **Testing & Rollout**
   - Run targeted self-play; review loss clusters for remaining gaps.
   - Document empirical impact in `docs/design/performance-analysis/AI_ENGINE_ANALYSIS.md`.

Risks & Mitigations
-------------------
- **Risk**: Over-penalizing creative lines resulting in monotonous play.  
  **Mitigation**: Keep penalties soft, scaling with search confirmation; allow overrides when PV shows tactical gain.
- **Risk**: Increased evaluation cost from new heuristics.  
  **Mitigation**: Profile SIMD benches; consider caching storm-state and castle-progress metrics.
- **Risk**: Opening book rigidity causing predictability.  
  **Mitigation**: Maintain multiple vetted templates and randomize selection within safe bounds.

Next Actions
------------
1. Implement redundant move and storm-detection scoring (owner: Evaluation subsystem).
2. Update opening book generation scripts with king-first bans (owner: Opening subsystem).
3. Script self-play regression from critical FENs; record outcomes in performance analysis doc (owner: Testing).
4. Re-run SIMD benches and note any regressions before merging changes.

