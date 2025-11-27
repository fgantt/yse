/// Validation tools for opening book
///
/// This module provides comprehensive validation of opening book data,
/// including duplicate detection, move legality, weight/evaluation consistency,
/// FEN format validation, and position bounds checking.
use super::OpeningBook;
use crate::opening_book::templates;
use std::collections::{BTreeMap, HashSet};

/// Validator for opening book data
pub struct BookValidator;

/// Validation report containing all validation results
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Number of duplicate positions found
    pub duplicates_found: usize,
    /// List of duplicate FEN strings
    pub duplicate_fens: Vec<String>,
    /// Number of illegal moves found
    pub illegal_moves: usize,
    /// List of illegal moves (FEN -> move description)
    pub illegal_move_details: Vec<(String, String)>,
    /// Number of weight/evaluation inconsistencies
    pub inconsistencies: usize,
    /// List of inconsistencies (FEN -> description)
    pub inconsistency_details: Vec<(String, String)>,
    /// Number of invalid FEN formats
    pub invalid_fen_count: usize,
    /// List of invalid FEN strings
    pub invalid_fens: Vec<String>,
    /// Number of positions out of bounds
    pub out_of_bounds_count: usize,
    /// List of out-of-bounds positions
    pub out_of_bounds_details: Vec<(String, String)>,
    /// General warnings
    pub warnings: Vec<String>,
    /// Whether validation passed (no errors)
    pub is_valid: bool,
    /// Number of king-first / rook-swing policy violations
    pub policy_violation_count: usize,
    /// Details for each policy violation
    pub policy_violation_details: Vec<(String, String)>,
    /// Number of openings that failed to map onto an approved template
    pub template_mismatch_count: usize,
    /// Details for each template mismatch
    pub template_mismatch_details: Vec<(String, String)>,
    /// Coverage summary by canonical template name
    pub template_summary: Vec<(String, usize)>,
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self {
            duplicates_found: 0,
            duplicate_fens: Vec::new(),
            illegal_moves: 0,
            illegal_move_details: Vec::new(),
            inconsistencies: 0,
            inconsistency_details: Vec::new(),
            invalid_fen_count: 0,
            invalid_fens: Vec::new(),
            out_of_bounds_count: 0,
            out_of_bounds_details: Vec::new(),
            warnings: Vec::new(),
            is_valid: true,
            policy_violation_count: 0,
            policy_violation_details: Vec::new(),
            template_mismatch_count: 0,
            template_mismatch_details: Vec::new(),
            template_summary: Vec::new(),
        }
    }
}

impl BookValidator {
    /// Validate for duplicate positions
    ///
    /// Checks if the same FEN string appears multiple times in the book.
    pub fn validate_duplicate_positions(book: &OpeningBook) -> (usize, Vec<String>) {
        let mut seen_fens: HashSet<String> = HashSet::new();
        let mut duplicates = Vec::new();

        let positions = book.get_all_positions();
        for (fen, _) in positions {
            if !seen_fens.insert(fen.clone()) {
                duplicates.push(fen);
            }
        }

        (duplicates.len(), duplicates)
    }

    /// Validate move legality
    ///
    /// Verifies that all book moves are legal moves from their positions.
    /// Note: This requires board state and engine integration, so it's a stub.
    pub fn validate_move_legality(_book: &OpeningBook) -> (usize, Vec<(String, String)>) {
        // This requires:
        // 1. Parsing FEN to board state
        // 2. Generating legal moves from position
        // 3. Checking if book move is in legal moves list
        //
        // For now, return empty results (stub implementation)
        (0, Vec::new())
    }

    /// Validate weight/evaluation consistency
    ///
    /// Checks that weights correlate with evaluations (high weight → high
    /// eval).
    pub fn validate_weight_evaluation_consistency(
        book: &OpeningBook,
    ) -> (usize, Vec<(String, String)>) {
        let mut inconsistencies = 0;
        let mut details = Vec::new();

        let positions = book.get_all_positions();
        for (fen, moves) in positions {
            if moves.len() < 2 {
                continue; // Need at least 2 moves to compare
            }

            // Find moves with high weight but low evaluation
            // or low weight but high evaluation
            for (i, move1) in moves.iter().enumerate() {
                for (j, move2) in moves.iter().enumerate() {
                    if i >= j {
                        continue;
                    }

                    // High weight should correlate with high evaluation
                    // Flag if weight difference is large but evaluation difference is opposite
                    let weight_diff = move1.weight as i32 - move2.weight as i32;
                    let eval_diff = move1.evaluation - move2.evaluation;

                    // If weight difference is significant (>200) but eval difference is opposite or
                    // small
                    if weight_diff.abs() > 200 {
                        if (weight_diff > 0 && eval_diff < -50)
                            || (weight_diff < 0 && eval_diff > 50)
                        {
                            inconsistencies += 1;
                            details.push((
                                fen.clone(),
                                format!(
                                    "Moves {} and {}: weight diff={}, eval diff={} (inconsistent)",
                                    i, j, weight_diff, eval_diff
                                ),
                            ));
                        }
                    }
                }
            }
        }

        (inconsistencies, details)
    }

    /// Validate FEN format
    ///
    /// Verifies that all FEN strings are valid Shogi FEN format.
    pub fn validate_fen_format(book: &OpeningBook) -> (usize, Vec<String>) {
        let mut invalid_count = 0;
        let mut invalid_fens = Vec::new();

        let positions = book.get_all_positions();
        for (fen, _) in positions {
            // Basic FEN format validation:
            // Shogi FEN format: "board position active_player captured_pieces move_number"
            // Should have at least 4 parts separated by spaces
            let parts: Vec<&str> = fen.split_whitespace().collect();
            if parts.len() < 4 {
                invalid_count += 1;
                invalid_fens.push(fen);
                continue;
            }

            // Check that first part (board) has 9 rows separated by '/'
            let board_parts: Vec<&str> = parts[0].split('/').collect();
            if board_parts.len() != 9 {
                invalid_count += 1;
                invalid_fens.push(fen);
                continue;
            }

            // Check that active player is 'b' or 'w'
            if parts.len() > 3
                && parts[3] != "b"
                && parts[3] != "w"
                && parts[3] != "B"
                && parts[3] != "W"
            {
                invalid_count += 1;
                invalid_fens.push(fen);
            }
        }

        (invalid_count, invalid_fens)
    }

    /// Validate position bounds
    ///
    /// Verifies that all positions (from/to) are within board bounds (0-8 for
    /// row/col).
    pub fn validate_position_bounds(book: &OpeningBook) -> (usize, Vec<(String, String)>) {
        let mut out_of_bounds = 0;
        let mut details = Vec::new();

        let positions = book.get_all_positions();
        for (fen, moves) in positions {
            for (i, book_move) in moves.iter().enumerate() {
                if let Some(from) = book_move.from {
                    if !from.is_valid() {
                        out_of_bounds += 1;
                        details.push((
                            fen.clone(),
                            format!("Move {}: invalid from position {:?}", i, from),
                        ));
                    }
                }

                if !book_move.to.is_valid() {
                    out_of_bounds += 1;
                    details.push((
                        fen.clone(),
                        format!("Move {}: invalid to position {:?}", i, book_move.to),
                    ));
                }
            }
        }

        (out_of_bounds, details)
    }

    /// Run full validation suite
    ///
    /// Executes all validation checks and returns a comprehensive report.
    pub fn run_full_validation(book: &OpeningBook) -> ValidationReport {
        let mut report = ValidationReport::default();

        // Run all validation checks
        let (duplicates, duplicate_fens) = Self::validate_duplicate_positions(book);
        report.duplicates_found = duplicates;
        report.duplicate_fens = duplicate_fens;

        let (illegal, illegal_details) = Self::validate_move_legality(book);
        report.illegal_moves = illegal;
        report.illegal_move_details = illegal_details;

        let (inconsistencies, inconsistency_details) =
            Self::validate_weight_evaluation_consistency(book);
        report.inconsistencies = inconsistencies;
        report.inconsistency_details = inconsistency_details;

        let (invalid_fen, invalid_fens) = Self::validate_fen_format(book);
        report.invalid_fen_count = invalid_fen;
        report.invalid_fens = invalid_fens;

        let (out_of_bounds, out_of_bounds_details) = Self::validate_position_bounds(book);
        report.out_of_bounds_count = out_of_bounds;
        report.out_of_bounds_details = out_of_bounds_details;

        let (policy_violations, policy_violation_details) = Self::validate_opening_policies(book);
        report.policy_violation_count = policy_violations;
        report.policy_violation_details = policy_violation_details;

        let (template_summary, template_mismatches) = Self::summarize_templates(book);
        report.template_summary = template_summary;
        report.template_mismatch_count = template_mismatches.len();
        report.template_mismatch_details = template_mismatches;

        // Determine if validation passed
        report.is_valid = report.duplicates_found == 0
            && report.illegal_moves == 0
            && report.inconsistencies == 0
            && report.invalid_fen_count == 0
            && report.out_of_bounds_count == 0
            && report.policy_violation_count == 0
            && report.template_mismatch_count == 0;

        // Add warnings if any issues found
        if report.duplicates_found > 0 {
            report
                .warnings
                .push(format!("Found {} duplicate positions", report.duplicates_found));
        }
        if report.illegal_moves > 0 {
            report.warnings.push(format!("Found {} illegal moves", report.illegal_moves));
        }
        if report.inconsistencies > 0 {
            report.warnings.push(format!(
                "Found {} weight/evaluation inconsistencies",
                report.inconsistencies
            ));
        }
        if report.invalid_fen_count > 0 {
            report
                .warnings
                .push(format!("Found {} invalid FEN formats", report.invalid_fen_count));
        }
        if report.out_of_bounds_count > 0 {
            report
                .warnings
                .push(format!("Found {} out-of-bounds positions", report.out_of_bounds_count));
        }
        if report.policy_violation_count > 0 {
            report
                .warnings
                .push(format!("Found {} policy violations", report.policy_violation_count));
        }
        if report.template_mismatch_count > 0 {
            report.warnings.push(format!(
                "Found {} openings without a registered template",
                report.template_mismatch_count
            ));
        }

        report
    }

    fn summarize_templates(book: &OpeningBook) -> (Vec<(String, usize)>, Vec<(String, String)>) {
        let mut summary: BTreeMap<String, usize> = BTreeMap::new();
        let mut mismatches = Vec::new();

        for (fen, moves) in book.get_all_positions() {
            for book_move in moves {
                match book_move.opening_name.as_deref() {
                    Some(name) => {
                        if let Some(template) = templates::find_template_for_opening(name) {
                            *summary.entry(template.canonical_name.to_string()).or_insert(0) += 1;
                        } else {
                            mismatches.push((
                                fen.clone(),
                                format!("Opening '{}' is missing from template registry", name),
                            ));
                        }
                    }
                    None => mismatches
                        .push((fen.clone(), "Opening metadata missing opening_name".to_string())),
                }
            }
        }

        (summary.into_iter().collect(), mismatches)
    }

    fn validate_opening_policies(book: &OpeningBook) -> (usize, Vec<(String, String)>) {
        let mut violations = 0;
        let mut details = Vec::new();

        for (fen, moves) in book.get_all_positions() {
            for book_move in moves {
                if let Some(reason) = OpeningBook::detect_policy_violation(&fen, &book_move) {
                    violations += 1;
                    details.push((fen.clone(), reason));
                }
            }
        }

        (violations, details)
    }
}
