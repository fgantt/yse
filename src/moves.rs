#[cfg(feature = "simd")]
use crate::bitboards::sliding_moves::SlidingMoveGenerator;
use crate::bitboards::*;
use crate::types::board::CapturedPieces;
use crate::types::core::{Move, Piece, PieceType, Player, Position};
use crate::types::Bitboard;

pub struct MoveGenerator {
    // Cache for move generation to avoid redundant work
    move_cache: std::collections::HashMap<String, Vec<Move>>,
    cache_hits: u64,
    cache_misses: u64,
    // Feature flags for magic bitboard integration
    magic_bitboard_enabled: bool,
    batch_processing_enabled: bool,
    // Performance metrics
    magic_move_count: u64,
    raycast_move_count: u64,
    magic_generation_time: std::time::Duration,
    raycast_generation_time: std::time::Duration,
    /// SIMD optimization configuration
    ///
    /// Controls runtime enabling/disabling of SIMD optimizations for move
    /// generation. Only effective when the `simd` feature is enabled at
    /// compile time.
    ///
    /// # Task 4.0 (Task 4.6)
    #[cfg(feature = "simd")]
    simd_config: crate::config::SimdConfig,
}

impl MoveGenerator {
    pub fn new() -> Self {
        Self {
            move_cache: std::collections::HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
            magic_bitboard_enabled: true,
            batch_processing_enabled: true,
            #[cfg(feature = "simd")]
            simd_config: crate::config::SimdConfig::default(),
            magic_move_count: 0,
            raycast_move_count: 0,
            magic_generation_time: std::time::Duration::ZERO,
            raycast_generation_time: std::time::Duration::ZERO,
        }
    }

    /// Create a new move generator with custom settings
    pub fn with_settings(magic_enabled: bool, batch_processing: bool) -> Self {
        Self {
            move_cache: std::collections::HashMap::new(),
            cache_hits: 0,
            cache_misses: 0,
            magic_bitboard_enabled: magic_enabled,
            batch_processing_enabled: batch_processing,
            magic_move_count: 0,
            raycast_move_count: 0,
            magic_generation_time: std::time::Duration::ZERO,
            raycast_generation_time: std::time::Duration::ZERO,
            #[cfg(feature = "simd")]
            simd_config: crate::config::SimdConfig::default(),
        }
    }

    /// Enable or disable magic bitboards
    pub fn set_magic_bitboard_enabled(&mut self, enabled: bool) {
        self.magic_bitboard_enabled = enabled;
    }

    /// Check if magic bitboards are enabled
    pub fn is_magic_bitboard_enabled(&self) -> bool {
        self.magic_bitboard_enabled
    }

    /// Set SIMD configuration
    ///
    /// # Task 4.0 (Task 4.6)
    #[cfg(feature = "simd")]
    pub fn set_simd_config(&mut self, config: crate::config::SimdConfig) {
        self.simd_config = config;
    }

    /// Get SIMD configuration
    ///
    /// # Task 4.0 (Task 4.6)
    #[cfg(feature = "simd")]
    pub fn simd_config(&self) -> &crate::config::SimdConfig {
        &self.simd_config
    }

    /// Enable or disable batch processing
    pub fn set_batch_processing_enabled(&mut self, enabled: bool) {
        self.batch_processing_enabled = enabled;
    }

    /// Check if batch processing is enabled
    pub fn is_batch_processing_enabled(&self) -> bool {
        self.batch_processing_enabled
    }

    /// Get performance comparison metrics
    pub fn get_performance_metrics(&self) -> MoveGenerationMetrics {
        MoveGenerationMetrics {
            magic_move_count: self.magic_move_count,
            raycast_move_count: self.raycast_move_count,
            magic_generation_time: self.magic_generation_time,
            raycast_generation_time: self.raycast_generation_time,
            magic_bitboard_enabled: self.magic_bitboard_enabled,
            batch_processing_enabled: self.batch_processing_enabled,
        }
    }

    /// Reset performance metrics
    pub fn reset_metrics(&mut self) {
        self.magic_move_count = 0;
        self.raycast_move_count = 0;
        self.magic_generation_time = std::time::Duration::ZERO;
        self.raycast_generation_time = std::time::Duration::ZERO;
    }

    pub fn generate_legal_moves(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        let is_in_check = board.is_king_in_check(player, captured_pieces);

        crate::debug_log_fast!(&format!(
            "[GENERATE_LEGAL_MOVES] Player: {:?}, In check: {}",
            player, is_in_check
        ));

        let pseudo_legal_moves = self.generate_pseudo_legal_moves(board, player, captured_pieces);
        crate::debug_log_fast!(&format!(
            "[GENERATE_LEGAL_MOVES] Generated {} pseudo-legal moves",
            pseudo_legal_moves.len()
        ));

        // A pseudo-legal move is legal unless it leaves the mover's king in check.
        // Test each on one scratch board with make/unmake: this used to clone the
        // board (and the hands, which the check test does not read) per move, the
        // single largest cost of move generation.
        //
        // The same test also sets `gives_check`, which pruning relies on to leave
        // checks alone; moves from this generator never had it set.
        //
        // Filtered in place, keeping order, so the list is allocated once.
        let mut scratch = board.clone();
        let mut legal_moves = pseudo_legal_moves;
        let filter = MoveTestFilter::new(board, player, is_in_check, captured_pieces);
        legal_moves.retain_mut(|m| {
            let (safe, check) = filter.test(&mut scratch, m, player, captured_pieces);
            m.gives_check = check;
            safe
        });

        crate::debug_log_fast!(&format!(
            "[GENERATE_LEGAL_MOVES] Final legal moves: {}",
            legal_moves.len()
        ));
        if is_in_check {
            crate::debug_log_fast!(&format!(
                "[GENERATE_LEGAL_MOVES] Player was in check, found {} legal moves",
                legal_moves.len()
            ));
        }

        legal_moves
    }

    /// Number of `player`'s legal board moves that capture, plus the number that
    /// promote (a capturing promotion counts twice), with no pieces in hand.
    ///
    /// Exactly what counting over `generate_legal_moves` with empty hands gives,
    /// without building that list or testing the legality of quiet moves,
    /// which contribute nothing: the search asks this for both sides at most
    /// nodes, and the full generation was ~30% of search time.
    pub fn count_legal_captures_and_promotions(&self, board: &BitboardBoard, player: Player) -> usize {
        let no_hands = CapturedPieces::new();
        let mut moves = Vec::with_capacity(160);
        self.generate_all_piece_moves_into(board, player, &mut moves);
        let in_check = board.is_king_in_check(player, &no_hands);
        let filter = MoveTestFilter::new(board, player, in_check, &no_hands);
        let mut scratch = board.clone();
        moves
            .iter()
            .filter(|m| m.is_capture || m.is_promotion)
            .filter(|m| filter.is_safe(&mut scratch, m, player, &no_hands))
            .map(|m| m.is_capture as usize + m.is_promotion as usize)
            .sum()
    }

    pub fn generate_legal_captures(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        let pseudo_legal_moves =
            self.generate_pseudo_legal_captures(board, player, captured_pieces);

        // Filter out moves that leave the king in check
        let mut scratch = board.clone();
        pseudo_legal_moves
            .into_iter()
            .filter(|m| king_safe_after(&mut scratch, m, player, captured_pieces))
            .collect()
    }

    fn generate_pseudo_legal_captures(
        &self,
        board: &BitboardBoard,
        player: Player,
        _captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        self.generate_capture_piece_moves(board, player)
    }

    fn generate_capture_piece_moves(&self, board: &BitboardBoard, player: Player) -> Vec<Move> {
        let mut moves = Vec::new();
        for r in 0..9 {
            for c in 0..9 {
                let pos = Position::new(r, c);
                if let Some(piece) = board.get_piece(pos) {
                    if piece.player == player {
                        moves.extend(self.generate_capture_moves_for_piece(board, &piece, pos));
                    }
                }
            }
        }
        moves
    }

    fn generate_capture_moves_for_piece(
        &self,
        board: &BitboardBoard,
        piece: &Piece,
        pos: Position,
    ) -> Vec<Move> {
        let mut moves = Vec::new();
        let player = piece.player;

        let handle_capture_move = |moves: &mut Vec<Move>, to_pos: Position| {
            if !board.is_square_occupied_by(to_pos, player) {
                if board.is_square_occupied(to_pos) {
                    // Is a capture
                    let from_in_opponent_promo = pos.is_in_promotion_zone(player.opposite());
                    let to_in_opponent_promo = to_pos.is_in_promotion_zone(player.opposite());

                    // Non-promoted move (not allowed where the piece could never move again)
                    if !must_promote(piece.piece_type, player, to_pos) {
                        let mut move_ =
                            Move::new_move(pos, to_pos, piece.piece_type, player, false);
                        move_.is_capture = true;
                        move_.captured_piece = board.get_piece(to_pos);
                        moves.push(move_);
                    }

                    // Promoted move
                    if piece.piece_type.can_promote()
                        && (from_in_opponent_promo || to_in_opponent_promo)
                    {
                        let mut promoted_move =
                            Move::new_move(pos, to_pos, piece.piece_type, player, true);
                        promoted_move.is_capture = true;
                        promoted_move.captured_piece = board.get_piece(to_pos);
                        moves.push(promoted_move);
                    }
                }
            }
        };

        match piece.piece_type {
            PieceType::Pawn => {
                let dir: i8 = if player == Player::Black { -1 } else { 1 };
                let new_row = pos.row as i8 + dir;
                if new_row >= 0 && new_row < 9 {
                    let target_pos = Position::new(new_row as u8, pos.col);
                    
                    // CRITICAL: Verify pawn direction is correct
                    #[cfg(debug_assertions)]
                    {
                        if player == Player::Black && new_row > pos.row as i8 {
                            panic!(
                                "PAWN MOVE BUG: Black pawn at row {} trying to move to row {} (upward movement is illegal for Black!)",
                                pos.row, new_row
                            );
                        }
                        if player == Player::White && new_row < pos.row as i8 {
                            panic!(
                                "PAWN MOVE BUG: White pawn at row {} trying to move to row {} (downward movement is illegal for White!)",
                                pos.row, new_row
                            );
                        }
                    }
                    
                    handle_capture_move(&mut moves, target_pos);
                }
            }
            PieceType::Knight => {
                let dir: i8 = if player == Player::Black { -1 } else { 1 };
                let move_offsets = [(2 * dir, 1), (2 * dir, -1)];
                for (dr, dc) in move_offsets.iter() {
                    let new_row = pos.row as i8 + dr;
                    let new_col = pos.col as i8 + dc;
                    if new_row >= 0 && new_row < 9 && new_col >= 0 && new_col < 9 {
                        handle_capture_move(
                            &mut moves,
                            Position::new(new_row as u8, new_col as u8),
                        );
                    }
                }
            }
            PieceType::Lance | PieceType::Rook | PieceType::Bishop => {
                let directions = match piece.piece_type {
                    PieceType::Lance => {
                        if player == Player::Black {
                            vec![(-1, 0)]
                        } else {
                            vec![(1, 0)]
                        }
                    }
                    PieceType::Rook => vec![(1, 0), (-1, 0), (0, 1), (0, -1)],
                    PieceType::Bishop => vec![(1, 1), (1, -1), (-1, 1), (-1, -1)],
                    _ => vec![],
                };

                for (dr, dc) in directions {
                    let mut current_pos = pos;
                    loop {
                        let new_row = current_pos.row as i8 + dr;
                        let new_col = current_pos.col as i8 + dc;
                        if new_row < 0 || new_row >= 9 || new_col < 0 || new_col >= 9 {
                            break;
                        }

                        current_pos = Position::new(new_row as u8, new_col as u8);
                        handle_capture_move(&mut moves, current_pos);

                        if board.is_square_occupied(current_pos) {
                            break;
                        }
                    }
                }
            }
            // Dragons and horses slide like a rook/bishop *and* step one square
            // diagonally/orthogonally. The precomputed tables are static, so they
            // hold only the steps — these pieces used to move like kings.
            // `get_attack_pattern` walks the rays against the current occupancy and
            // includes the extra steps.
            PieceType::PromotedBishop | PieceType::PromotedRook => {
                let attacks = board.get_attack_pattern(pos, piece.piece_type);
                use crate::bitboards::integration::GlobalOptimizer;
                let mut remaining = attacks;
                while let Some(target_square) = GlobalOptimizer::bit_scan_forward(remaining) {
                    handle_capture_move(&mut moves, Position::from_index(target_square));
                    remaining &= Bitboard::from_u128(remaining.to_u128() - 1);
                }
            }
            PieceType::Silver
            | PieceType::Gold
            | PieceType::King
            | PieceType::PromotedPawn
            | PieceType::PromotedLance
            | PieceType::PromotedKnight
            | PieceType::PromotedSilver => {
                let dir: i8 = if player == Player::Black { -1 } else { 1 };
                let offsets = piece.piece_type.get_move_offsets(dir);
                for (dr, dc) in offsets {
                    let new_row = pos.row as i8 + dr;
                    let new_col = pos.col as i8 + dc;
                    if new_row >= 0 && new_row < 9 && new_col >= 0 && new_col < 9 {
                        handle_capture_move(
                            &mut moves,
                            Position::new(new_row as u8, new_col as u8),
                        );
                    }
                }
            }
        }
        moves
    }

    fn generate_pseudo_legal_moves(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        // Shogi positions routinely exceed a hundred legal moves, and drops add
        // many more; growing from empty reallocated and memmoved this list
        // several times per node.
        let mut moves = Vec::with_capacity(192);
        self.generate_all_piece_moves_into(board, player, &mut moves);
        self.generate_drop_moves_into(board, player, captured_pieces, &mut moves);
        moves
    }

    /// Generate moves for all pieces on the board
    ///
    /// Uses SIMD-optimized batch generation for sliding pieces when the `simd`
    /// feature is enabled, falling back to scalar implementation otherwise.
    ///
    /// # Performance
    ///
    /// When SIMD is enabled, sliding pieces (rook, bishop, lance) are processed
    /// in batches using vectorized operations, achieving 2-4x speedup over
    /// scalar implementation.
    ///
    /// # Memory Optimizations (Task 3.12)
    ///
    /// This method includes several memory optimizations when SIMD is enabled:
    /// - **Prefetching**: Prefetches upcoming magic table entries for better
    ///   cache utilization
    /// - **Batch processing**: Processes sliding pieces in batches for improved
    ///   cache locality
    /// - **Sequential prefetching**: Prefetches next pieces in batch ahead of
    ///   time
    ///
    /// These optimizations provide an additional 5-10% performance improvement
    /// on top of SIMD optimizations.
    pub fn generate_all_piece_moves(&self, board: &BitboardBoard, player: Player) -> Vec<Move> {
        let mut moves = Vec::with_capacity(160);
        self.generate_all_piece_moves_into(board, player, &mut moves);
        moves
    }

    /// [`Self::generate_all_piece_moves`], appending to `moves`. The search
    /// calls this once per node, so it allocates nothing of its own: the piece
    /// lists live on the stack (a side has at most 40 pieces, inside 64 slots) and every
    /// generator writes straight into the caller's list.
    pub fn generate_all_piece_moves_into(
        &self,
        board: &BitboardBoard,
        player: Player,
        moves: &mut Vec<Move>,
    ) {
        #[cfg(feature = "simd")]
        {
            // Collect pieces by type for batch processing
            let mut sliding_pieces: smallvec::SmallVec<[(Position, Piece); 8]> =
                smallvec::SmallVec::new();
            let mut non_sliding_pieces: smallvec::SmallVec<[(Position, Piece); 64]> =
                smallvec::SmallVec::new();

            for r in 0..9 {
                for c in 0..9 {
                    let pos = Position::new(r, c);
                    if let Some(piece) = board.get_piece(pos) {
                        if piece.player == player {
                            // Check if this is a sliding piece that can use SIMD batch processing
                            match piece.piece_type {
                                PieceType::Rook
                                | PieceType::Bishop
                                | PieceType::PromotedRook
                                | PieceType::PromotedBishop => {
                                    sliding_pieces.push((pos, piece));
                                }
                                _ => {
                                    non_sliding_pieces.push((pos, piece));
                                }
                            }
                        }
                    }
                }
            }

            // Use SIMD batch generation for sliding pieces if magic table is available and
            // SIMD is enabled
            if !sliding_pieces.is_empty() && self.simd_config.enable_simd_move_generation {
                // Try to get magic table from board
                let magic_table = board.get_magic_table();

                if let Some(magic_table) = magic_table {
                    // Record SIMD move generation call
                    crate::utils::telemetry::SIMD_TELEMETRY.record_simd_move_gen();
                    let sliding_generator = SlidingMoveGenerator::new(magic_table);
                    sliding_generator.generate_sliding_moves_batch_vectorized_into(
                        board,
                        &sliding_pieces,
                        player,
                        moves,
                    );
                } else {
                    // Record scalar move generation call
                    crate::utils::telemetry::SIMD_TELEMETRY.record_scalar_move_gen();
                    // Fallback: use scalar generation if magic table not available
                    for (pos, piece) in &sliding_pieces {
                        self.generate_moves_for_single_piece_into(board, piece, *pos, moves);
                    }
                }
            } else {
                // Record scalar move generation call
                crate::utils::telemetry::SIMD_TELEMETRY.record_scalar_move_gen();
                // Fallback: use scalar generation if SIMD disabled or no sliding pieces
                for (pos, piece) in &sliding_pieces {
                    self.generate_moves_for_single_piece_into(board, piece, *pos, moves);
                }
            }

            // Generate moves for non-sliding pieces using existing logic
            for (pos, piece) in non_sliding_pieces {
                self.generate_moves_for_single_piece_into(board, &piece, pos, moves);
            }
        }

        #[cfg(not(feature = "simd"))]
        {
            // Scalar implementation (fallback when SIMD feature is disabled)
            // Note: No telemetry tracking when SIMD feature is disabled
            for r in 0..9 {
                for c in 0..9 {
                    let pos = Position::new(r, c);
                    if let Some(piece) = board.get_piece(pos) {
                        if piece.player == player {
                            self.generate_moves_for_single_piece_into(board, &piece, pos, moves);
                        }
                    }
                }
            }
        }
    }

    /// Append this piece's moves to `out`.
    ///
    /// Callers immediately fold the result into a larger list, so returning a
    /// fresh `Vec` here meant one allocation and one free per piece per node —
    /// malloc/free and the memmove behind `raw_vec::finish_grow` were ~15% of
    /// search time. Appending in place removes that traffic.
    fn generate_moves_for_single_piece_into(
        &self,
        board: &BitboardBoard,
        piece: &Piece,
        pos: Position,
        out: &mut Vec<Move>,
    ) {
        let player = piece.player;

        let handle_move = |moves: &mut Vec<Move>, to_pos: Position| {
            if !board.is_square_occupied_by(to_pos, player) {
                let is_capture = board.is_square_occupied(to_pos);
                let from_in_opponent_promo = pos.is_in_promotion_zone(player.opposite());
                let to_in_opponent_promo = to_pos.is_in_promotion_zone(player.opposite());

                // Non-promoted move (not allowed where the piece could never move again)
                if !must_promote(piece.piece_type, player, to_pos) {
                    let mut move_ = Move::new_move(pos, to_pos, piece.piece_type, player, false);
                    if is_capture {
                        move_.is_capture = true;
                        move_.captured_piece = board.get_piece(to_pos);
                    }
                    moves.push(move_);
                }

                // Promoted move
                if piece.piece_type.can_promote()
                    && (from_in_opponent_promo || to_in_opponent_promo)
                {
                    let mut promoted_move =
                        Move::new_move(pos, to_pos, piece.piece_type, player, true);
                    if is_capture {
                        promoted_move.is_capture = true;
                        promoted_move.captured_piece = board.get_piece(to_pos);
                    }
                    moves.push(promoted_move);
                }
            }
        };

        match piece.piece_type {
            PieceType::Pawn => {
                let dir: i8 = if player == Player::Black { -1 } else { 1 };
                let new_row = pos.row as i8 + dir;
                if new_row >= 0 && new_row < 9 {
                    let target_pos = Position::new(new_row as u8, pos.col);
                    
                    // CRITICAL: Verify pawn direction is correct
                    #[cfg(debug_assertions)]
                    {
                        if player == Player::Black && new_row > pos.row as i8 {
                            panic!(
                                "PAWN MOVE BUG: Black pawn at row {} trying to move to row {} (upward movement is illegal for Black!)",
                                pos.row, new_row
                            );
                        }
                        if player == Player::White && new_row < pos.row as i8 {
                            panic!(
                                "PAWN MOVE BUG: White pawn at row {} trying to move to row {} (downward movement is illegal for White!)",
                                pos.row, new_row
                            );
                        }
                    }
                    
                    handle_move(out, target_pos);
                }
            }
            PieceType::Knight => {
                // Use precomputed attack patterns for better performance
                let attacks = board.get_attack_pattern_precomputed(pos, piece.piece_type, player);

                // Task 3.0.3.3: Convert attack bitboard to moves using bit scans instead of
                // 0..81 loop
                use crate::bitboards::integration::GlobalOptimizer;
                let mut remaining = attacks;
                while !remaining.is_empty() {
                    if let Some(target_square) = GlobalOptimizer::bit_scan_forward(remaining) {
                        let target_pos = Position::from_index(target_square);
                        handle_move(out, target_pos);
                        remaining &= Bitboard::from_u128(remaining.to_u128() - 1);
                    } else {
                        break;
                    }
                }
            }
            PieceType::Lance | PieceType::Rook | PieceType::Bishop => {
                let directions = match piece.piece_type {
                    PieceType::Lance => {
                        if player == Player::Black {
                            vec![(-1, 0)]
                        } else {
                            vec![(1, 0)]
                        }
                    }
                    PieceType::Rook => vec![(1, 0), (-1, 0), (0, 1), (0, -1)],
                    PieceType::Bishop => vec![(1, 1), (1, -1), (-1, 1), (-1, -1)],
                    _ => vec![],
                };

                for (dr, dc) in directions {
                    let mut current_pos = pos;
                    loop {
                        let new_row = current_pos.row as i8 + dr;
                        let new_col = current_pos.col as i8 + dc;
                        if new_row < 0 || new_row >= 9 || new_col < 0 || new_col >= 9 {
                            break;
                        }

                        current_pos = Position::new(new_row as u8, new_col as u8);
                        handle_move(out, current_pos);

                        if board.is_square_occupied(current_pos) {
                            break;
                        }
                    }
                }
            }
            // Dragons and horses slide like a rook/bishop *and* step one square
            // diagonally/orthogonally. The precomputed tables are static, so they
            // hold only the steps — these pieces used to move like kings.
            // `get_attack_pattern` walks the rays against the current occupancy and
            // includes the extra steps.
            PieceType::PromotedBishop | PieceType::PromotedRook => {
                let attacks = board.get_attack_pattern(pos, piece.piece_type);
                use crate::bitboards::integration::GlobalOptimizer;
                let mut remaining = attacks;
                while let Some(target_square) = GlobalOptimizer::bit_scan_forward(remaining) {
                    handle_move(out, Position::from_index(target_square));
                    remaining &= Bitboard::from_u128(remaining.to_u128() - 1);
                }
            }
            PieceType::Silver
            | PieceType::Gold
            | PieceType::King
            | PieceType::PromotedPawn
            | PieceType::PromotedLance
            | PieceType::PromotedKnight
            | PieceType::PromotedSilver => {
                // Use precomputed attack patterns for better performance
                let attacks = board.get_attack_pattern_precomputed(pos, piece.piece_type, player);

                // Task 3.0.3.3: Convert attack bitboard to moves using bit scans instead of
                // 0..81 loop
                use crate::bitboards::integration::GlobalOptimizer;
                let mut remaining = attacks;
                while !remaining.is_empty() {
                    if let Some(target_square) = GlobalOptimizer::bit_scan_forward(remaining) {
                        let target_pos = Position::from_index(target_square);
                        handle_move(out, target_pos);
                        remaining &= Bitboard::from_u128(remaining.to_u128() - 1);
                    } else {
                        break;
                    }
                }
            }
        }
    }

    /// Convenience wrapper for callers that want an owned list.
    fn generate_moves_for_single_piece(
        &self,
        board: &BitboardBoard,
        piece: &Piece,
        pos: Position,
    ) -> Vec<Move> {
        let mut out = Vec::new();
        self.generate_moves_for_single_piece_into(board, piece, pos, &mut out);
        out
    }

    /// Append every legal drop square for each piece type in hand, in the order
    /// the types first appear in the hand list.
    ///
    /// Runs once per node. It used to build a `HashSet` to skip repeated types
    /// and a `HashMap` of counts that nothing read -- two allocations and a
    /// SipHash per piece in hand; a 14-entry flag array does the first job.
    fn generate_drop_moves_into(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
        moves: &mut Vec<Move>,
    ) {
        let mut processed = [false; PieceType::COUNT];
        let captured =
            if player == Player::Black { &captured_pieces.black } else { &captured_pieces.white };

        for &piece_type in captured {
            if std::mem::replace(&mut processed[piece_type.as_index()], true) {
                continue;
            }
            
            // CRITICAL: Verify piece is actually in hand
            let count = captured_pieces.count(piece_type, player);
            if count == 0 {
                #[cfg(debug_assertions)]
                {
                    eprintln!("DROP GENERATION BUG: Attempting to generate drops for piece type {:?} but count is 0!", piece_type);
                    eprintln!("  Player: {:?}", player);
                    eprintln!("  Captured pieces: {:?}", captured_pieces);
                    panic!("DROP GENERATION BUG: Piece type {:?} has count 0 but is in captured list!", piece_type);
                }
                continue; // Skip if not in hand
            }

            for r in 0..9 {
                for c in 0..9 {
                    let pos = Position::new(r, c);
                    
                    // Skip occupied squares (this is expected - most squares are occupied)
                    if board.is_square_occupied(pos) {
                        continue;
                    }
                    
                    // Basic legality check for drops (e.g., pawn drops)
                    if is_legal_drop_location(board, piece_type, pos, player) {
                        // CRITICAL: Double-check piece is still in hand before creating move
                        let current_count = captured_pieces.count(piece_type, player);
                        if current_count == 0 {
                            #[cfg(debug_assertions)]
                            {
                                eprintln!("DROP GENERATION BUG: Piece count became 0 during move generation!");
                                eprintln!("  Piece type: {:?}", piece_type);
                                eprintln!("  Player: {:?}", player);
                                eprintln!("  Position: row {}, col {}", r, c);
                                panic!("DROP GENERATION BUG: Piece count is 0 when generating drop move!");
                            }
                            continue; // Skip if not in hand
                        }
                        
                        let drop_move = Move::new_drop(piece_type, pos, player);
                        
                        // Note: is_legal_drop_location already checks the two-pawn rule,
                        // so we don't need to check again here
                        
                        moves.push(drop_move);
                    }
                }
            }
        }
    }

    /// Generate all moves that give check to the opponent
    pub fn generate_checks(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        let mut check_moves = Vec::new();
        let opponent = player.opposite();

        // Generate all pseudo-legal moves
        let all_moves = self.generate_pseudo_legal_moves(board, player, captured_pieces);

        for mut move_ in all_moves {
            // Make the move on a temporary board
            let mut temp_board = board.clone();
            let mut temp_captured = captured_pieces.clone();

            temp_board.make_move_with_hand(&move_, &mut temp_captured);

            // Check if this move gives check to the opponent
            if temp_board.is_king_in_check(opponent, &temp_captured) {
                move_.gives_check = true;
                check_moves.push(move_);
            }
        }

        check_moves
    }

    /// Generate all promotion moves
    pub fn generate_promotions(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        let mut promotion_moves = Vec::new();

        // Generate all pseudo-legal moves
        let all_moves = self.generate_pseudo_legal_moves(board, player, captured_pieces);

        for move_ in all_moves {
            if move_.is_promotion {
                promotion_moves.push(move_);
            }
        }

        promotion_moves
    }

    /// Generate moves that create tactical threats
    pub fn generate_tactical_threats(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        let mut threat_moves = Vec::new();
        let _opponent = player.opposite();

        // Generate all pseudo-legal moves
        let all_moves = self.generate_pseudo_legal_moves(board, player, captured_pieces);

        for move_ in all_moves {
            // Check if this move creates a threat (attacks opponent pieces or creates
            // tactical patterns)
            if self.is_tactical_threat(&move_, board, player) {
                threat_moves.push(move_);
            }
        }

        threat_moves
    }

    /// Check if a move creates a tactical threat
    fn is_tactical_threat(&self, move_: &Move, board: &BitboardBoard, player: Player) -> bool {
        // For now, we'll consider moves that attack opponent pieces as threats
        // This can be expanded to include more sophisticated threat detection
        if move_.is_capture {
            return true;
        }

        // Check if the move attacks opponent pieces
        if let Some(from) = move_.from {
            if let Some(piece) = board.get_piece(from) {
                // Check if this piece can attack opponent pieces from the new position
                let opponent = player.opposite();
                let mut temp_board = board.clone();
                temp_board.remove_piece(from);
                temp_board.place_piece(piece, move_.to);

                // Check if the piece can attack any opponent pieces from this position
                for r in 0..9 {
                    for c in 0..9 {
                        let pos = Position::new(r, c);
                        if let Some(target_piece) = board.get_piece(pos) {
                            if target_piece.player == opponent {
                                // This is a simplified threat detection
                                // In a more sophisticated implementation, we would check
                                // if the piece can actually attack the target
                                return true;
                            }
                        }
                    }
                }
            }
        }

        false
    }

    /// Generate all quiescence moves (captures, checks, promotions, threats)
    pub fn generate_quiescence_moves(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        // Pre-allocate with estimated capacity to reduce allocations
        let mut moves = Vec::with_capacity(32);

        // 1. Generate captures (highest priority) - most important for quiescence
        let captures = self.generate_legal_captures(board, player, captured_pieces);
        moves.extend(captures);

        // 2. Generate checks - high priority for tactical positions
        let checks = self.generate_checks(board, player, captured_pieces);
        moves.extend(checks);

        // 3. Generate promotions - important for endgame tactics
        let promotions = self.generate_promotions(board, player, captured_pieces);
        moves.extend(promotions);

        // 4. Generate tactical threats - only if we have few moves so far
        if moves.len() < 16 {
            // Only generate threats if we don't have many tactical moves
            let threats = self.generate_tactical_threats(board, player, captured_pieces);
            moves.extend(threats);
        }

        // Remove duplicates efficiently and sort by priority
        self.deduplicate_and_sort_quiescence_moves(&mut moves);
        moves
    }

    /// Optimized deduplication and sorting for quiescence moves
    /// Bounded quiescence moves: captures and promotions, plus checks only when
    /// `include_checks` (the search passes `true` at the first quiescence ply only).
    ///
    /// `generate_quiescence_moves` also adds every checking move — including
    /// drops, of which there can be dozens with pieces in hand — and, when fewer
    /// than 16 moves were found, quiet "tactical threat" moves, at *every*
    /// quiescence ply. That turns quiescence into a second full-width search: a
    /// single depth-2 search made ~846k evaluations. This is the standard,
    /// bounded policy.
    pub fn generate_quiescence_moves_bounded(
        &self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
        include_checks: bool,
    ) -> Vec<Move> {
        let mut moves = Vec::with_capacity(32);
        moves.extend(self.generate_legal_captures(board, player, captured_pieces));
        moves.extend(self.generate_promotions(board, player, captured_pieces));
        if include_checks {
            moves.extend(self.generate_checks(board, player, captured_pieces));
        }
        self.deduplicate_and_sort_quiescence_moves(&mut moves);
        moves
    }

    fn deduplicate_and_sort_quiescence_moves(&self, moves: &mut Vec<Move>) {
        if moves.is_empty() {
            return;
        }

        // Sort first to group duplicates together
        // Use indexed approach to ensure deterministic ordering even for identical moves
        let mut indices: Vec<usize> = (0..moves.len()).collect();
        indices.sort_by(|&idx_a, &idx_b| {
            let cmp = self.compare_quiescence_moves_simple(&moves[idx_a], &moves[idx_b]);
            if cmp != std::cmp::Ordering::Equal {
                cmp
            } else {
                // Final tie-breaker: use original index to ensure deterministic ordering
                idx_a.cmp(&idx_b)
            }
        });
        // Reorder moves based on sorted indices
        let mut new_moves = Vec::with_capacity(moves.len());
        for idx in indices {
            new_moves.push(moves[idx].clone());
        }
        *moves = new_moves;

        // Remove duplicates (moves with same from, to, and piece_type)
        let mut write_index = 0;
        for read_index in 1..moves.len() {
            if !self.moves_equal(&moves[write_index], &moves[read_index]) {
                write_index += 1;
                if write_index != read_index {
                    moves[write_index] = moves[read_index].clone();
                }
            }
        }
        moves.truncate(write_index + 1);
    }

    /// Check if two moves are equal (for deduplication)
    fn moves_equal(&self, a: &Move, b: &Move) -> bool {
        a.from == b.from && a.to == b.to && a.piece_type == b.piece_type && a.player == b.player
    }

    /// Generate quiescence moves with caching
    pub fn generate_quiescence_moves_cached(
        &mut self,
        board: &BitboardBoard,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> Vec<Move> {
        // Create cache key from board state
        let cache_key = format!("q_{}_{}", board.to_fen(player, captured_pieces), player as u8);

        // Check cache first
        if let Some(cached_moves) = self.move_cache.get(&cache_key) {
            self.cache_hits += 1;
            return cached_moves.clone();
        }

        self.cache_misses += 1;

        // Generate moves if not in cache
        let moves = self.generate_quiescence_moves(board, player, captured_pieces);

        // Cache the result (limit cache size)
        if self.move_cache.len() < 1000 {
            self.move_cache.insert(cache_key, moves.clone());
        }

        moves
    }

    /// Clear the move cache
    pub fn clear_cache(&mut self) {
        self.move_cache.clear();
        self.cache_hits = 0;
        self.cache_misses = 0;
    }

    /// Get cache statistics
    pub fn get_cache_stats(&self) -> (u64, u64, f64) {
        let total_attempts = self.cache_hits + self.cache_misses;
        let hit_rate = if total_attempts > 0 {
            (self.cache_hits as f64 / total_attempts as f64) * 100.0
        } else {
            0.0
        };
        (self.cache_hits, self.cache_misses, hit_rate)
    }

    /// Simple comparison function for quiescence moves (used in MoveGenerator)
    pub fn compare_quiescence_moves_simple(&self, a: &Move, b: &Move) -> std::cmp::Ordering {
        // Create a simple, guaranteed total order by using a hash-based comparison
        // This ensures we never have equal moves that are actually different

        // 1. Checks first
        match (a.gives_check, b.gives_check) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }

        // 2. Captures vs non-captures (captures have higher priority)
        match (a.is_capture, b.is_capture) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            (true, true) => {
                // Both are captures - use MVV-LVA
                let a_value = a.captured_piece_value() - a.piece_value();
                let b_value = b.captured_piece_value() - b.piece_value();
                let capture_cmp = b_value.cmp(&a_value);
                if capture_cmp != std::cmp::Ordering::Equal {
                    return capture_cmp;
                }
            }
            (false, false) => {
                // Neither is a capture - continue to other criteria
            }
        }

        // 3. Promotions
        match (a.is_promotion, b.is_promotion) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }

        // 4. Tactical threat assessment
        let a_threat_value = self.assess_tactical_threat_value(a);
        let b_threat_value = self.assess_tactical_threat_value(b);
        let threat_cmp = b_threat_value.cmp(&a_threat_value);
        if threat_cmp != std::cmp::Ordering::Equal {
            return threat_cmp;
        }

        // 5. Piece value ordering
        let piece_cmp = b.piece_value().cmp(&a.piece_value());
        if piece_cmp != std::cmp::Ordering::Equal {
            return piece_cmp;
        }

        // 6. Use a simple hash-based comparison to ensure total order
        // This guarantees that different moves will always have different orderings
        let a_hash = self.move_hash(a);
        let b_hash = self.move_hash(b);
        let hash_cmp = a_hash.cmp(&b_hash);
        if hash_cmp != std::cmp::Ordering::Equal {
            return hash_cmp;
        }
        
        // Final tie-breaker: compare moves directly by their properties
        // This ensures total order even if hash collides (extremely unlikely)
        self.compare_moves_directly(a, b)
    }

    /// Compare moves directly by their properties to ensure total order
    /// This is used as a final tie-breaker when all other comparisons are equal
    fn compare_moves_directly(&self, a: &Move, b: &Move) -> std::cmp::Ordering {
        // Compare by to position first
        let to_cmp = a.to.row.cmp(&b.to.row);
        if to_cmp != std::cmp::Ordering::Equal {
            return to_cmp;
        }
        let to_col_cmp = a.to.col.cmp(&b.to.col);
        if to_col_cmp != std::cmp::Ordering::Equal {
            return to_col_cmp;
        }
        
        // Compare by from position
        match (a.from, b.from) {
            (Some(a_from), Some(b_from)) => {
                let from_row_cmp = a_from.row.cmp(&b_from.row);
                if from_row_cmp != std::cmp::Ordering::Equal {
                    return from_row_cmp;
                }
                let from_col_cmp = a_from.col.cmp(&b_from.col);
                if from_col_cmp != std::cmp::Ordering::Equal {
                    return from_col_cmp;
                }
            }
            (Some(_), None) => return std::cmp::Ordering::Less,
            (None, Some(_)) => return std::cmp::Ordering::Greater,
            (None, None) => {}
        }
        
        // Compare by piece type
        let piece_cmp = (a.piece_type as u8).cmp(&(b.piece_type as u8));
        if piece_cmp != std::cmp::Ordering::Equal {
            return piece_cmp;
        }
        
        // Compare by player
        let player_cmp = (a.player as u8).cmp(&(b.player as u8));
        if player_cmp != std::cmp::Ordering::Equal {
            return player_cmp;
        }
        
        // Final tie-breaker: compare by move flags to ensure total order
        // Even if all other properties are equal, these flags can differ
        // This ensures we never return Equal for different moves
        let a_flags = ((a.is_promotion as u8) << 2) | ((a.is_capture as u8) << 1) | (a.gives_check as u8);
        let b_flags = ((b.is_promotion as u8) << 2) | ((b.is_capture as u8) << 1) | (b.gives_check as u8);
        let flags_cmp = a_flags.cmp(&b_flags);
        if flags_cmp != std::cmp::Ordering::Equal {
            return flags_cmp;
        }
        
        // If we get here, the moves are truly identical
        // Return Equal (this is fine - identical moves should compare as equal)
        std::cmp::Ordering::Equal
    }

    /// Create a simple hash for move comparison
    fn move_hash(&self, move_: &Move) -> u64 {
        let mut hash = 0u64;

        // Hash the to position
        hash = hash.wrapping_mul(31).wrapping_add(move_.to.row as u64);
        hash = hash.wrapping_mul(31).wrapping_add(move_.to.col as u64);

        // Hash the from position (if exists)
        if let Some(from) = move_.from {
            hash = hash.wrapping_mul(31).wrapping_add(from.row as u64);
            hash = hash.wrapping_mul(31).wrapping_add(from.col as u64);
        }

        // Hash the piece type
        hash = hash.wrapping_mul(31).wrapping_add(move_.piece_type as u64);

        // Hash the player
        hash = hash.wrapping_mul(31).wrapping_add(move_.player as u64);

        hash
    }

    /// Compare two moves for quiescence search ordering
    #[allow(dead_code)]
    fn compare_quiescence_moves(&self, a: &Move, b: &Move) -> std::cmp::Ordering {
        // 1. Checks first
        match (a.gives_check, b.gives_check) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }

        // 2. MVV-LVA for captures
        if a.is_capture && b.is_capture {
            let a_value = a.captured_piece_value() - a.piece_value();
            let b_value = b.captured_piece_value() - b.piece_value();
            let capture_cmp = b_value.cmp(&a_value);
            if capture_cmp != std::cmp::Ordering::Equal {
                return capture_cmp;
            }
        }

        // 3. Promotions
        match (a.is_promotion, b.is_promotion) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }

        // 4. Tactical threat assessment
        let a_threat_value = self.assess_tactical_threat_value(a);
        let b_threat_value = self.assess_tactical_threat_value(b);
        let threat_cmp = b_threat_value.cmp(&a_threat_value);
        if threat_cmp != std::cmp::Ordering::Equal {
            return threat_cmp;
        }

        // 5. Piece value ordering
        let piece_cmp = b.piece_value().cmp(&a.piece_value());
        if piece_cmp != std::cmp::Ordering::Equal {
            return piece_cmp;
        }

        // 6. Position-based ordering (to ensure total order)
        let a_pos_value = (a.to.row as i32 * 9 + a.to.col as i32) as i32;
        let b_pos_value = (b.to.row as i32 * 9 + b.to.col as i32) as i32;
        let pos_cmp = a_pos_value.cmp(&b_pos_value);
        if pos_cmp != std::cmp::Ordering::Equal {
            return pos_cmp;
        }

        // 7. From position ordering
        match (a.from, b.from) {
            (Some(a_from), Some(b_from)) => {
                let a_from_value = (a_from.row as i32 * 9 + a_from.col as i32) as i32;
                let b_from_value = (b_from.row as i32 * 9 + b_from.col as i32) as i32;
                a_from_value.cmp(&b_from_value)
            }
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    }

    /// Assess tactical threat value for move ordering
    fn assess_tactical_threat_value(&self, move_: &Move) -> i32 {
        let mut threat_value = 0;

        // High value for captures
        if move_.is_capture {
            threat_value += move_.captured_piece_value();
        }

        // High value for checks
        if move_.gives_check {
            threat_value += 1000;
        }

        // High value for promotions
        if move_.is_promotion {
            threat_value += move_.promotion_value();
        }

        // High value for recaptures
        if move_.is_recapture {
            threat_value += 500;
        }

        threat_value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitboards::BitboardBoard;

    /// `MoveTestFilter` must give exactly `test_after`'s answers, for every
    /// pseudo-legal move of every position along random games -- which pass
    /// through checks, captures, drops and promotions.
    #[test]
    fn move_test_filter_matches_full_test() {
        use rand::{seq::SliceRandom, SeedableRng};
        let mg = MoveGenerator::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(31);
        let (mut compared, mut checks, mut unsafe_moves) = (0u64, 0u64, 0u64);
        for _ in 0..40 {
            let mut board = BitboardBoard::new();
            let mut captured = CapturedPieces::new();
            let mut player = Player::Black;
            for _ in 0..160 {
                let in_check = board.is_king_in_check(player, &captured);
                let filter = MoveTestFilter::new(&board, player, in_check, &captured);
                let mut scratch = board.clone();
                for m in mg.generate_pseudo_legal_moves(&board, player, &captured) {
                    let full = test_after(&mut scratch, &m, player, &captured);
                    let fast = filter.test(&mut scratch, &m, player, &captured);
                    assert_eq!(fast, full, "{} in {}", m.to_usi_string(), board.to_fen(player, &captured));
                    compared += 1;
                    checks += full.1 as u64;
                    unsafe_moves += (!full.0) as u64;
                }
                let legal = mg.generate_legal_moves(&board, player, &captured);
                let Some(m) = legal.choose(&mut rng) else { break };
                board.make_move_with_hand(m, &mut captured);
                player = player.opposite();
            }
        }
        // The positions must actually exercise both answers.
        assert!(checks > 100 && unsafe_moves > 100, "{compared} {checks} {unsafe_moves}");
    }

    /// `count_legal_captures_and_promotions` must equal the count it replaced:
    /// captures plus promotions over `generate_legal_moves` with empty hands.
    #[test]
    fn capture_promotion_count_matches_full_generation() {
        use rand::{seq::SliceRandom, SeedableRng};
        let mg = MoveGenerator::new();
        let no_hands = CapturedPieces::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(32);
        let mut nonzero = 0;
        for _ in 0..30 {
            let mut board = BitboardBoard::new();
            let mut captured = CapturedPieces::new();
            let mut player = Player::Black;
            for _ in 0..160 {
                for side in [Player::Black, Player::White] {
                    let expected: usize = mg
                        .generate_legal_moves(&board, side, &no_hands)
                        .iter()
                        .map(|m| m.is_capture as usize + m.is_promotion as usize)
                        .sum();
                    let got = mg.count_legal_captures_and_promotions(&board, side);
                    assert_eq!(got, expected, "{:?} in {}", side, board.to_fen(player, &captured));
                    nonzero += (expected > 0) as usize;
                }
                let legal = mg.generate_legal_moves(&board, player, &captured);
                let Some(m) = legal.choose(&mut rng) else { break };
                board.make_move_with_hand(m, &mut captured);
                player = player.opposite();
            }
        }
        assert!(nonzero > 1000, "{nonzero}");
    }
    use crate::types::Bitboard;
    use crate::types::{CapturedPieces, Piece, PieceType, Player, Position};

    #[test]
    fn test_white_pawn_promotion() {
        let fen = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPP1PP/1B5R1/LNSGKGSNL w - 1";
        let (board, player, captured_pieces) = BitboardBoard::from_fen(fen).unwrap();
        let move_generator = MoveGenerator::new();
        let moves = move_generator.generate_legal_moves(&board, player, &captured_pieces);

        for m in &moves {
            if m.to_usi_string().contains("+") {
                assert!(
                    m.to.is_in_promotion_zone(player)
                        || m.from.unwrap().is_in_promotion_zone(player)
                );
            }
        }
    }

    #[test]
    fn test_quiescence_move_sorting_total_order() {
        let move_generator = MoveGenerator::new();

        // Create test moves with different properties
        let mut test_moves = vec![
            // Non-capture move
            Move {
                from: Some(Position { row: 1, col: 1 }),
                to: Position { row: 2, col: 1 },
                piece_type: PieceType::Pawn,
                player: Player::Black,
                is_capture: false,
                is_promotion: false,
                gives_check: false,
                is_recapture: false,
                captured_piece: None,
            },
            // Capture move
            Move {
                from: Some(Position { row: 1, col: 2 }),
                to: Position { row: 2, col: 2 },
                piece_type: PieceType::Pawn,
                player: Player::Black,
                is_capture: true,
                is_promotion: false,
                gives_check: false,
                is_recapture: false,
                captured_piece: Some(Piece { piece_type: PieceType::Pawn, player: Player::White }),
            },
            // Check move
            Move {
                from: Some(Position { row: 1, col: 3 }),
                to: Position { row: 2, col: 3 },
                piece_type: PieceType::Pawn,
                player: Player::Black,
                is_capture: false,
                is_promotion: false,
                gives_check: true,
                is_recapture: false,
                captured_piece: None,
            },
        ];

        // Test that sorting doesn't panic and produces consistent results
        test_moves.sort_by(|a, b| move_generator.compare_quiescence_moves_simple(a, b));

        // Verify the ordering is correct
        // Check should be first, then capture, then non-capture
        assert!(test_moves[0].gives_check, "Check move should be first");
        assert!(test_moves[1].is_capture, "Capture move should be second");
        assert!(
            !test_moves[2].is_capture && !test_moves[2].gives_check,
            "Non-capture move should be last"
        );

        // Test that the comparison is transitive and consistent
        for i in 0..test_moves.len() {
            for j in 0..test_moves.len() {
                let cmp_ij =
                    move_generator.compare_quiescence_moves_simple(&test_moves[i], &test_moves[j]);
                let cmp_ji =
                    move_generator.compare_quiescence_moves_simple(&test_moves[j], &test_moves[i]);

                // Test antisymmetry: if a < b, then b > a
                match (cmp_ij, cmp_ji) {
                    (std::cmp::Ordering::Less, std::cmp::Ordering::Greater) => {}
                    (std::cmp::Ordering::Greater, std::cmp::Ordering::Less) => {}
                    (std::cmp::Ordering::Equal, std::cmp::Ordering::Equal) => {}
                    _ => panic!("Comparison is not antisymmetric: {} vs {}", i, j),
                }
            }
        }
    }
}
/// A pawn or lance on the last rank, or a knight on the last two, could never
/// move again, so moving one there must promote. Drops there are excluded
/// separately, in [`is_legal_drop_location`].
///
/// Without this the generator offered the non-promoting move too. The engine
/// could play it, and a GUI or teacher engine rejects it as illegal (found when
/// YaneuraOu refused a random `6b6a` pawn move during corpus generation).
fn must_promote(piece_type: PieceType, player: Player, to: Position) -> bool {
    let rank = if player == Player::Black { to.row } else { 8 - to.row };
    match piece_type {
        PieceType::Pawn | PieceType::Lance => rank == 0,
        PieceType::Knight => rank <= 1,
        _ => false,
    }
}

/// Whether `player`'s king is safe after `m`, tested on `scratch` (a copy of the
/// current board) and undone before returning.
fn king_safe_after(
    scratch: &mut BitboardBoard,
    m: &Move,
    player: Player,
    captured_pieces: &CapturedPieces,
) -> bool {
    test_after(scratch, m, player, captured_pieces).0
}

/// Squares from which some shogi piece can attack square `k`, on an empty
/// board: the rook and bishop lines through `k` plus the knight jumps of either
/// colour. Every piece's attack set -- steps, jumps, slides, lances -- lies
/// inside it.
const fn reach_masks() -> ([u128; 81], [u128; 81]) {
    let mut lines = [0u128; 81];
    let mut reach = [0u128; 81];
    let dirs: [(i8, i8); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
    let knights: [(i8, i8); 4] = [(2, 1), (2, -1), (-2, 1), (-2, -1)];
    let mut k = 0;
    while k < 81 {
        let (r, c) = ((k / 9) as i8, (k % 9) as i8);
        let mut d = 0;
        while d < 8 {
            let (mut rr, mut cc) = (r + dirs[d].0, c + dirs[d].1);
            while rr >= 0 && rr < 9 && cc >= 0 && cc < 9 {
                lines[k] |= 1u128 << (rr as usize * 9 + cc as usize);
                rr += dirs[d].0;
                cc += dirs[d].1;
            }
            d += 1;
        }
        reach[k] = lines[k];
        let mut j = 0;
        while j < 4 {
            let (rr, cc) = (r + knights[j].0, c + knights[j].1);
            if rr >= 0 && rr < 9 && cc >= 0 && cc < 9 {
                reach[k] |= 1u128 << (rr as usize * 9 + cc as usize);
            }
            j += 1;
        }
        k += 1;
    }
    (lines, reach)
}

/// `.0`: rook/bishop lines through each square. `.1`: those plus knight jumps.
static REACH: ([u128; 81], [u128; 81]) = reach_masks();

/// [`test_after`] with the attack tests it can answer without making the move.
///
/// The legality filter ran two full attack tests per pseudo-legal move, ~23%
/// of search time. Most of them have a known answer:
///
/// * If the mover is not in check, a move can expose its king only by moving
///   the king or by vacating a square on a line through it (a discovered
///   attack). Occupying `to` only blocks, and a capture leaves `to` occupied.
/// * Likewise, if the opponent is not in check, a move can check it only by
///   landing where some piece could attack its king (`REACH`), or by vacating a
///   square on a line through it.
///
/// Anything else -- either side already in check, a missing king -- takes the
/// full test, so the answers are exactly those of [`test_after`].
struct MoveTestFilter {
    /// Own king square, when not in check (otherwise every move is tested).
    own_king: Option<usize>,
    own_in_check: bool,
    /// Opponent king square, when it is not in check.
    opp_king: Option<usize>,
    opp_in_check: bool,
}

impl MoveTestFilter {
    fn new(
        board: &BitboardBoard,
        player: Player,
        own_in_check: bool,
        captured_pieces: &CapturedPieces,
    ) -> Self {
        let opp_in_check = board.is_king_in_check(player.opposite(), captured_pieces);
        Self {
            own_king: board.find_king_position(player).map(|p| p.to_index() as usize),
            own_in_check,
            opp_king: board.find_king_position(player.opposite()).map(|p| p.to_index() as usize),
            opp_in_check,
        }
    }

    fn test(
        &self,
        scratch: &mut BitboardBoard,
        m: &Move,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> (bool, bool) {
        let from_bit = m.from.map_or(0, |f| 1u128 << f.to_index());
        let to_bit = 1u128 << m.to.to_index();
        // No king means `is_king_in_check` answers false: safe, and no check.
        let safety_known = match self.own_king {
            None => true,
            Some(k) => {
                !self.own_in_check
                    && m.piece_type != PieceType::King
                    && REACH.0[k] & from_bit == 0
            }
        };
        let check_known = match self.opp_king {
            None => true,
            Some(k) => !self.opp_in_check && REACH.1[k] & to_bit == 0 && REACH.0[k] & from_bit == 0,
        };
        if safety_known && check_known {
            return (true, false);
        }
        let info = scratch.make_move_with_info(m);
        let safe = safety_known || !scratch.is_king_in_check(player, captured_pieces);
        let check =
            safe && !check_known && scratch.is_king_in_check(player.opposite(), captured_pieces);
        scratch.unmake_move(&info);
        (safe, check)
    }

    /// The first half of [`Self::test`] alone: is `player`'s king safe after `m`.
    fn is_safe(
        &self,
        scratch: &mut BitboardBoard,
        m: &Move,
        player: Player,
        captured_pieces: &CapturedPieces,
    ) -> bool {
        let known = match self.own_king {
            None => true,
            Some(k) => {
                !self.own_in_check
                    && m.piece_type != PieceType::King
                    && REACH.0[k] & m.from.map_or(0, |f| 1u128 << f.to_index()) == 0
            }
        };
        if known {
            return true;
        }
        let info = scratch.make_move_with_info(m);
        let safe = !scratch.is_king_in_check(player, captured_pieces);
        scratch.unmake_move(&info);
        safe
    }
}

/// After `m` on `scratch` (undone before returning): (is `player`'s king safe,
/// is the opponent's king in check).
fn test_after(
    scratch: &mut BitboardBoard,
    m: &Move,
    player: Player,
    captured_pieces: &CapturedPieces,
) -> (bool, bool) {
    let info = scratch.make_move_with_info(m);
    let safe = !scratch.is_king_in_check(player, captured_pieces);
    let check = safe && scratch.is_king_in_check(player.opposite(), captured_pieces);
    scratch.unmake_move(&info);
    (safe, check)
}

fn is_legal_drop_location(
    board: &BitboardBoard,
    piece_type: PieceType,
    pos: Position,
    player: Player,
) -> bool {
    if piece_type == PieceType::Pawn {
        // Rule 1: Cannot drop on a file that already contains an unpromoted pawn of the
        // same color (Nifu / 二歩)
        let file = 9 - pos.col;
        let mut pawn_count = 0;
        let mut pawn_positions = Vec::new();
        
        for r in 0..9 {
            let check_pos = Position::new(r, pos.col);
            if let Some(p) = board.get_piece(check_pos) {
                if p.piece_type == PieceType::Pawn && p.player == player {
                    pawn_count += 1;
                    pawn_positions.push((r, check_pos));
                }
            }
        }
        
        if pawn_count > 0 {
            // Two-pawn rule violation - this is normal and expected, just return false
            // Only log in debug mode if we want to trace this
            crate::debug_log_fast!(&format!(
                "[NIFU] Illegal pawn drop at {}{}. Already have {} pawn(s) on file {}",
                (b'a' + pos.col) as char,
                9 - pos.row,
                pawn_count,
                (b'a' + pos.col) as char
            ));
            return false;
        }

        // Rule 2: Cannot drop pawn to give immediate checkmate (Uchifuzume /
        // 打ち歩詰め) This rule only applies to drops that give checkmate, not
        // just check
        if is_pawn_drop_mate(board, pos, player) {
            crate::debug_log_fast!(&format!(
                "[UCHIFUZUME] Illegal pawn drop mate at {}{}",
                (b'a' + pos.col) as char,
                9 - pos.row
            ));
            return false;
        }
    }

    // Cannot drop a piece where it has no legal moves
    let last_rank = if player == Player::Black { 0 } else { 8 };
    let second_last_rank = if player == Player::Black { 1 } else { 7 };
    match piece_type {
        PieceType::Pawn | PieceType::Lance if pos.row == last_rank => return false,
        PieceType::Knight if pos.row == last_rank || pos.row == second_last_rank => return false,
        _ => true,
    }
}

/// Check if dropping a pawn at the given position gives immediate checkmate
/// (Uchifuzume) This is illegal in Shogi - you cannot drop a pawn to deliver
/// checkmate
fn is_pawn_drop_mate(board: &BitboardBoard, drop_pos: Position, player: Player) -> bool {
    // Find opponent's king
    let opponent = player.opposite();
    let Some(king_pos) = board.find_king_position(opponent) else {
        return false; // No king, can't be checkmate
    };

    // Check if the pawn would give check
    let pawn_gives_check = match player {
        Player::Black => {
            // Black pawn attacks one square forward (decreasing row)
            king_pos.row == drop_pos.row.wrapping_sub(1) && king_pos.col == drop_pos.col
        }
        Player::White => {
            // White pawn attacks one square forward (increasing row)
            king_pos.row == drop_pos.row + 1 && king_pos.col == drop_pos.col
        }
    };

    if !pawn_gives_check {
        return false; // Not even giving check, so not checkmate
    }

    // Checkmate iff the defender has no legal reply. A pawn check is adjacent,
    // so it cannot be blocked and a drop can never answer it: the only replies
    // are board moves (the king stepping away or taking the pawn, or another
    // piece taking the pawn). Generating board moves alone also keeps this from
    // recursing through the defender's own pawn drops.
    //
    // This used to check king steps only, so any checking pawn drop the king
    // could not dodge counted as mate even when another piece could take the
    // pawn, and the engine refused legal moves strong engines play.
    let mut after_drop = board.clone();
    after_drop.place_piece(Piece::new(PieceType::Pawn, player), drop_pos);
    let no_hand = CapturedPieces::new();
    let replies = MoveGenerator::new().generate_all_piece_moves(&after_drop, opponent);
    !replies.iter().any(|reply| {
        let mut b = after_drop.clone();
        b.make_move(reply);
        !b.is_king_in_check(opponent, &no_hand)
    })
}

/// Performance metrics for move generation
#[derive(Debug, Clone)]
pub struct MoveGenerationMetrics {
    pub magic_move_count: u64,
    pub raycast_move_count: u64,
    pub magic_generation_time: std::time::Duration,
    pub raycast_generation_time: std::time::Duration,
    pub magic_bitboard_enabled: bool,
    pub batch_processing_enabled: bool,
}

impl MoveGenerationMetrics {
    /// Calculate the speedup ratio of magic bitboards over ray-casting
    pub fn magic_speedup_ratio(&self) -> f64 {
        if self.raycast_generation_time.as_nanos() > 0 {
            self.raycast_generation_time.as_nanos() as f64
                / self.magic_generation_time.as_nanos() as f64
        } else {
            1.0
        }
    }

    /// Calculate the efficiency of magic bitboards
    pub fn magic_efficiency(&self) -> f64 {
        if self.magic_move_count + self.raycast_move_count > 0 {
            self.magic_move_count as f64 / (self.magic_move_count + self.raycast_move_count) as f64
        } else {
            0.0
        }
    }

    /// Get average time per move for magic bitboards
    pub fn magic_avg_time_per_move(&self) -> std::time::Duration {
        if self.magic_move_count > 0 {
            self.magic_generation_time / self.magic_move_count as u32
        } else {
            std::time::Duration::ZERO
        }
    }

    /// Get average time per move for ray-casting
    pub fn raycast_avg_time_per_move(&self) -> std::time::Duration {
        if self.raycast_move_count > 0 {
            self.raycast_generation_time / self.raycast_move_count as u32
        } else {
            std::time::Duration::ZERO
        }
    }
}
