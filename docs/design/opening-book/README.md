# Opening Book Stability Notes

The gameplay-stability tasks mandate additional validation around the opening book. Key points:

- **Template registry** – Book moves must advertise an `opening_name` that maps to one of the
  canonical templates defined in `src/opening_book/templates.rs`. The canonical policy is documented
  in `../gameplay-stability/OPENING_TEMPLATE_POLICY.md`.
- **Policy checks** – `OpeningBook::detect_policy_violation` prevents king-first continuations and
  premature rook swings unless the owning template explicitly allows them (e.g., Ranging Rook rook
  shifts).
- **Validation hooks** – `BookValidator::summarize_templates` emits coverage stats and fails fast when
  a move references an unknown template.

When regenerating `src/ai/openingBook.json`, always run the validator so these constraints are
applied before shipping a new book blob.




