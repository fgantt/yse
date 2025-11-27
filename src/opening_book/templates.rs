//! Opening template registry
//!
//! Provides canonical template metadata so the book compiler can bucket
//! concrete openings (Yagura, Anaguma, etc.) into the stability templates
//! described in `docs/design/gameplay-stability/OPENING_TEMPLATE_POLICY.md`.
//! The registry powers:
//! - Guardrails such as king-first and early rook-swing bans.
//! - Reporting (template coverage summaries in validation reports).
//! - Future tuning knobs that need to differentiate static vs ranging plans.

use crate::types::core::PieceType;

/// Canonical gameplay-stability template surfaced in documentation.
#[derive(Debug, Clone, Copy)]
pub struct OpeningTemplate {
    /// Friendly template name (Static Rook, Ranging Rook, Disciplined Ureshino)
    pub canonical_name: &'static str,
    /// Short description for reports/tooling.
    pub description: &'static str,
    /// Concrete book opening names that map to this template.
    pub aliases: &'static [&'static str],
    /// Minimum ply (inclusive) before king moves are considered policy-safe.
    pub king_move_min_ply: u32,
    /// Minimum ply (inclusive) before lateral rook moves (rook swings) are allowed.
    pub rook_swing_min_ply: u32,
    /// Moves the template expects to see early (USI-like strings, e.g., "7g7f").
    pub priority_moves: &'static [&'static str],
}

impl OpeningTemplate {
    /// Returns true if moving `piece_type` before `ply` would violate template guidelines.
    pub fn violates_policy(&self, piece_type: PieceType, ply: u32, is_rook_swing: bool) -> bool {
        match piece_type {
            PieceType::King => ply < self.king_move_min_ply,
            PieceType::Rook => is_rook_swing && ply < self.rook_swing_min_ply,
            _ => false,
        }
    }
}

const STATIC_ROOK_TEMPLATE: OpeningTemplate = OpeningTemplate {
    canonical_name: "Static Rook",
    description: "Classical static-rook shells (Yagura, Anaguma, Ibisha). Requires 7g7f and 2g2f \
         pawn pushes before king/rook maneuvers.",
    aliases: &["Yagura", "Anaguma", "Ibisha", "Central Pawn", "Side Pawn"],
    king_move_min_ply: 6,  // after both pawn pushes + one consolidation ply
    rook_swing_min_ply: 5, // after 7g7f + 2g2f have time to land
    priority_moves: &["7g7f", "2g2f"],
};

const RANGING_ROOK_TEMPLATE: OpeningTemplate = OpeningTemplate {
    canonical_name: "Ranging Rook",
    description:
        "Rook-ranging systems (Fourth File Rook, Quick Attack, Bishop Exchange, Ai Funibisha). \
         Early rook swings are expected once the 7g pawn advances.",
    aliases: &["Ranging Rook", "Quick Attack", "Bishop Exchange", "Ai Funibisha"],
    king_move_min_ply: 6,
    rook_swing_min_ply: 2, // rook can range immediately as part of the plan
    priority_moves: &["7g7f"],
};

const DISCIPLINED_URESHINO_TEMPLATE: OpeningTemplate = OpeningTemplate {
    canonical_name: "Disciplined Ureshino",
    description:
        "Stability-focused Ureshino branches. King steps only after the right-edge pawn push \
         has started and a defensive gold is mobilized.",
    aliases: &["Disciplined Ureshino"],
    king_move_min_ply: 4,
    rook_swing_min_ply: 5,
    priority_moves: &["7g7f", "6h6g"],
};

/// Returns the canonical template for a concrete opening name.
pub fn find_template_for_opening(opening_name: &str) -> Option<&'static OpeningTemplate> {
    templates()
        .iter()
        .find(|template| {
            template.aliases.iter().any(|alias| alias.eq_ignore_ascii_case(opening_name))
        })
}

/// Returns every registered template (for reporting / iteration).
pub const fn templates() -> &'static [OpeningTemplate] {
    &[STATIC_ROOK_TEMPLATE, RANGING_ROOK_TEMPLATE, DISCIPLINED_URESHINO_TEMPLATE]
}

/// Returns a fallback template for openings that do not advertise an alias (should be rare).
pub fn default_template() -> &'static OpeningTemplate {
    &STATIC_ROOK_TEMPLATE
}
