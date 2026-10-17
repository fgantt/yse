# Archive

Documentation written for earlier versions of the engine, kept for reference. **It does not
describe the current engine**: much of it predates the NNUE evaluation, the lean search
(`src/search/lean.rs`) and the history move ordering, and some describes options, binaries or
results that have since changed. For current behaviour see `docs/ENGINE_FEATURES.md` and the
latest session log in `docs/nnue-phase2/`.

| path | from |
|---|---|
| `ENGINE_CONFIGURATION_GUIDE.md`, `ENGINE_UTILITIES_GUIDE.md`, `UTILITIES_QUICK_REFERENCE.md` | Nov 2025: USI options and utility binaries of the classical engine. Many binaries still exist; check `--help` |
| `NNUE_*.md` | Dec 2025: the first NNUE integration and the online self-play trainer (`nnue_trainer`), since superseded by `nnue-offline-trainer` |
| `design/` | Nov 2025: feature designs, PRDs and task lists for the classical engine (some still cited from source comments) |
| `api/`, `performance/`, `tuning/`, `move-ordering/` | Nov 2025: API and tuning guides for the classical engine's subsystems |
| `nnue-phase2-plan-2026-04/` | Apr 2026: the research and implementation plan that opened the NNUE sessions |
| `agent-memory-2026-04/` | Apr 2026: architecture notes for AI agents (formerly `memory/`), written before the lean search |

The classical engine itself is the `legacy-main` branch (tag `pre-nnue-main`).
