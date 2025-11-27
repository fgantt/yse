# Opening Template Policy

The gameplay-stability effort collapses concrete book lines into three canonical templates so we can
enforce the same safety rails across evaluation, book compilation, and validation. The registry lives
in `src/opening_book/templates.rs` and must stay in sync with this document.

| Template | Aliases (book `opening_name`) | Early priorities | Guardrails |
| --- | --- | --- | --- |
| Static Rook | `Yagura`, `Anaguma`, `Ibisha`, `Central Pawn`, `Side Pawn` | `7g7f`, `2g2f` | King moves **≥ ply 6**; rook swings only after both pawn pushes (≥ ply 5). |
| Ranging Rook | `Ranging Rook`, `Quick Attack`, `Bishop Exchange`, `Ai Funibisha` | `7g7f` plus immediate rook shifts | King moves **≥ ply 6**; rook swings allowed from ply 2 (part of the plan). |
| Disciplined Ureshino | `Disciplined Ureshino` (future book entries) | `7g7f`, `6h6g` (gold mobilization) | King moves **≥ ply 4**; rook swings **≥ ply 5** to prevent reckless improvisations. |

## Policy Enforcement

1. **King-first ban** – `OpeningBook::detect_policy_violation` rejects any king move that violates the
   template-specific minimum ply. This is surfaced during book compilation and via
   `BookValidator::validate_opening_policies`.
2. **Early rook-swing guardrail** – Lateral rook moves (same rank, different file) are blocked until
   the template's `rook_swing_min_ply`. Ranging Rook is exempt so its rook can range immediately.
3. **Template coverage reporting** – `BookValidator::summarize_templates` tallies how many moves land
   inside each canonical template and flags any `opening_name` that lacks an alias mapping.
4. **Evaluation priors** – `OpeningPrincipleEvaluator::evaluate_opening_priors` awards bonuses when
   the static-rook pawn pushes materialize within the configured window (`opening_prior_window`).

## Regenerating / Extending the Book

When adding new positions or aliases:

1. Update `src/opening_book/templates.rs` with the alias list, ply thresholds, and priority moves.
2. Re-run the JSON converter (`cargo run --bin opening_book_converter …`) so new entries inherit the
   metadata.
3. Execute `BookValidator::run_full_validation` (or `cargo test -p shogi_engine
   book_validator::run_full_validation` via the existing harness) to confirm there are zero policy
   violations and that the template summary includes the new alias.
4. Document any new regeneration steps or template variants back in this file so search/move
   ordering teams have a single source of truth.

For a condensed pointer aimed at the opening-book tooling, see
`docs/design/opening-book/README.md`.

