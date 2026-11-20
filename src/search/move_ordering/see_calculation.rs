//! Static Exchange Evaluation (SEE) calculation
//!
//! This module contains the SEE calculation implementation.
//! SEE evaluates the material gain/loss from a sequence of captures
//! starting with the given move.

use crate::bitboards::BitboardBoard;
use crate::types::core::{Move, Piece, PieceType, Player, Position};
use std::collections::HashMap;

#[cfg(test)]
thread_local! {
    /// Tests: compute SEE's slider rays by ray-casting even when a magic table
    /// is loaded, to check the no-table path against the table.
    static FORCE_RAYCAST: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// SEE calculation result
pub type SEEResult<T> = Result<T, String>;

/// Find all attackers and defenders of a given square
///
/// This function identifies all pieces that can attack the target square.
/// For SEE calculation, we need to know which pieces can capture on this
/// square.
///
/// Returns a vector of all pieces that can attack the square, with their
/// positions. The caller will separate them by player.
/// Task 3.0.3.1: Rewritten to use bitboard iteration instead of nested 9×9
/// loops Task 3.0.3.4: Uses iter_pieces for efficient iteration over board
/// pieces
pub fn find_attackers_defenders(square: Position, board: &BitboardBoard) -> Vec<(Position, Piece)> {
    AttackList::attackers_of(square, board).as_slice().to_vec()
}

/// Whether `piece` standing on `from_pos` attacks `target_pos`, from the
/// board's attack tables (sliders along rays under the current occupancy).
pub fn piece_attacks_square(
    piece: &Piece,
    from_pos: Position,
    target_pos: Position,
    board: &BitboardBoard,
) -> bool {
    let player = piece.player;
    if from_pos.row >= 9 || from_pos.col >= 9 || target_pos.row >= 9 || target_pos.col >= 9 {
        return false;
    }
    let target_bit = crate::types::Bitboard::from_u128(1u128 << target_pos.to_index());
    let step = |pt: PieceType| {
        !(board.get_attack_pattern_precomputed(from_pos, pt, player) & target_bit).is_empty()
    };
    match piece.piece_type {
        PieceType::Lance => {
            // Towards the opponent: decreasing rows for Black.
            let dir: i8 = if player == Player::Black { -1 } else { 1 };
            check_ray_attack(from_pos, target_pos, (dir, 0), board)
        }
        PieceType::Rook | PieceType::PromotedRook => {
            [(1, 0), (-1, 0), (0, 1), (0, -1)]
                .iter()
                .any(|&d| check_ray_attack(from_pos, target_pos, d, board))
                || (piece.piece_type == PieceType::PromotedRook && step(PieceType::King))
        }
        PieceType::Bishop | PieceType::PromotedBishop => {
            [(1, 1), (1, -1), (-1, 1), (-1, -1)]
                .iter()
                .any(|&d| check_ray_attack(from_pos, target_pos, d, board))
                || (piece.piece_type == PieceType::PromotedBishop && step(PieceType::King))
        }
        pt => step(pt),
    }
}

/// Check if a ray from from_pos in direction (dr, dc) hits target_pos
fn check_ray_attack(
    from_pos: Position,
    target_pos: Position,
    direction: (i8, i8),
    board: &BitboardBoard,
) -> bool {
    let (dr, dc) = direction;
    let mut current_pos = from_pos;

    loop {
        let new_row = current_pos.row as i8 + dr;
        let new_col = current_pos.col as i8 + dc;

        // Out of bounds
        if new_row < 0 || new_row >= 9 || new_col < 0 || new_col >= 9 {
            break;
        }

        current_pos = Position::new(new_row as u8, new_col as u8);

        // Found target
        if current_pos == target_pos {
            return true;
        }

        // Blocked by a piece
        if board.is_square_occupied(current_pos) {
            break;
        }
    }

    false
}

/// Calculate Static Exchange Evaluation (SEE) for a move
///
/// This function simulates the sequence of captures that would follow
/// the given move and returns the net material gain/loss.
///
/// # Arguments
/// * `move_` - The move to evaluate
/// * `board` - The current board position
///
/// # Returns
/// The net material gain/loss from the exchange sequence

/// Sliding attackers of `to` under a hypothetical `occupied`, as (position, piece).
///
/// The exchange removes one piece from the board at a time, and each removal can
/// open a line: a rook behind the bishop that just captured now bears on the
/// square. Recomputing from the magic table under the reduced occupancy is what
/// makes those x-ray attackers visible. Without it an exchange defended "twice"
/// through a single file looks defended once, and SEE calls a losing capture
/// winning.
#[cfg(test)]
fn sliding_attackers(
    board: &BitboardBoard,
    to: Position,
    occupied: crate::types::Bitboard,
) -> Vec<(Position, Piece)> {
    let Some(magic) = board.magic_table_ref() else {
        return Vec::new(); // No table: callers fall back to the static list.
    };
    let to_idx = to.to_index();
    let rook_rays = magic.get_attacks(to_idx, PieceType::Rook, occupied);
    let bishop_rays = magic.get_attacks(to_idx, PieceType::Bishop, occupied);

    let mut found = Vec::new();
    let pieces = board.get_pieces();
    for player in [Player::Black, Player::White] {
        for pt in [
            PieceType::Rook,
            PieceType::PromotedRook,
            PieceType::Bishop,
            PieceType::PromotedBishop,
            PieceType::Lance,
        ] {
            // A lance only ever moves up its own file, so the rook rays bound
            // it; the direction is checked below.
            let rays = match pt {
                PieceType::Bishop | PieceType::PromotedBishop => bishop_rays,
                _ => rook_rays,
            };
            let mut bb = pieces[player as usize][pt.as_index()] & rays & occupied;
            while let Some(sq) = crate::bitboards::integration::GlobalOptimizer::bit_scan_forward(bb)
            {
                bb &= crate::types::Bitboard::from_u128(bb.to_u128() - 1);
                let pos = Position::from_index(sq);
                if pt == PieceType::Lance {
                    // Same file, and in front of the target from its own side.
                    if pos.col != to.col {
                        continue;
                    }
                    let forward = if player == Player::Black { pos.row > to.row } else { pos.row < to.row };
                    if !forward {
                        continue;
                    }
                }
                found.push((pos, Piece::new(pt, player)));
            }
        }
    }
    found
}

pub fn calculate_see_internal(move_: &Move, board: &BitboardBoard) -> i32 {
    let Some(captured) = move_.captured_piece.as_ref() else {
        return 0; // No capture, no SEE value
    };
    swap_value(move_, board, captured.piece_type.base_value())
}

/// Exchange value of a quiet move or drop: 0 if the piece is safe on its new
/// square, otherwise minus the material the opponent nets by capturing it
/// (and whatever exchange follows). A drop onto a square the opponent attacks
/// and we do not defend scores minus the dropped piece.
///
/// Capturing moves use [`calculate_see_internal`].
pub fn see_quiet(move_: &Move, board: &BitboardBoard) -> i32 {
    swap_value(move_, board, 0)
}

/// Attackers of a square in SEE order, on the stack. Mirrors the `Vec` the
/// exchange used to build -- same order, same deduplication, same removal by
/// shifting -- so the exchange picks exactly the same pieces.
#[derive(Clone, Copy)]
struct AttackList {
    items: [(Position, Piece); 48],
    len: usize,
    /// Squares ever listed, plus the target. A listed piece leaves the list
    /// only when it captures, and its square then leaves the occupancy the
    /// sliders are found under, so for an occupied square "seen" is "listed".
    seen: u128,
}

impl AttackList {
    fn new(square: Position) -> Self {
        Self {
            items: [(Position::new(0, 0), Piece::new(PieceType::Pawn, Player::Black)); 48],
            len: 0,
            seen: 1u128 << square.to_index(),
        }
    }

    #[inline]
    fn as_slice(&self) -> &[(Position, Piece)] {
        &self.items[..self.len]
    }

    #[inline]
    fn push(&mut self, item: (Position, Piece)) {
        // At most 40 pieces exist; the bound is never reached.
        if self.len < self.items.len() {
            self.items[self.len] = item;
            self.len += 1;
            self.seen |= 1u128 << item.0.to_index();
        }
    }

    /// Appends `player`'s `pt` on the squares of `bb` not yet seen, in square order.
    #[inline]
    fn push_all(&mut self, bb: crate::types::Bitboard, player: Player, pt: PieceType) {
        let mut v = bb.to_u128() & !self.seen;
        while v != 0 {
            let sq = v.trailing_zeros() as u8;
            v &= v - 1;
            self.push((Position::from_index(sq), Piece::new(pt, player)));
        }
    }

    #[cfg(test)]
    fn remove(&mut self, idx: usize) -> (Position, Piece) {
        let item = self.items[idx];
        self.items.copy_within(idx + 1..self.len, idx);
        self.len -= 1;
        item
    }

    /// Every piece of either colour attacking `square`, least valuable first
    /// (a stable sort, so equal values keep discovery order).
    fn attackers_of(square: Position, board: &BitboardBoard) -> Self {
        // Built on the board's own attack tables -- the ones `is_square_attacked_by`
        // uses and the board oracle tests cover. The hand-written version this
        // replaces had pawns, lances and knights moving the wrong way for both
        // colours and gave golds, silvers and promoted minors a king's eight
        // squares, so SEE saw attackers that were not there and missed ones that
        // were.
        let pieces = board.get_pieces();
        let mut found = Self::new(square);
        for player in [Player::Black, Player::White] {
            let own = &pieces[player as usize];
            // Step attacks, looked up from the target: pieces are point-symmetric
            // between the colours, so `player`'s pieces that attack `square` sit on
            // the squares a same-type piece of the other colour attacks from it.
            for pt in [
                PieceType::Pawn,
                PieceType::Knight,
                PieceType::Silver,
                PieceType::Gold,
                PieceType::PromotedPawn,
                PieceType::PromotedLance,
                PieceType::PromotedKnight,
                PieceType::PromotedSilver,
            ] {
                let bb = own[pt.as_index()];
                if bb.is_empty() {
                    continue;
                }
                found.push_all(
                    board.get_attack_pattern_precomputed(square, pt, player.opposite()) & bb,
                    player,
                    pt,
                );
            }
            // The king, and the one-step moves of dragon and horse.
            let adjacent = board.get_attack_pattern_precomputed(square, PieceType::King, player);
            for pt in [PieceType::King, PieceType::PromotedRook, PieceType::PromotedBishop] {
                found.push_all(adjacent & own[pt.as_index()], player, pt);
            }
        }
        // Sliders (rook, bishop, dragon, horse rays, lance) under the current occupancy.
        found.add_sliders(board, square, board.get_occupied_bitboard());
        // Least valuable first for SEE: a stable insertion sort.
        for i in 1..found.len {
            let item = found.items[i];
            let v = item.1.piece_type.base_value();
            let mut j = i;
            while j > 0 && found.items[j - 1].1.piece_type.base_value() > v {
                found.items[j] = found.items[j - 1];
                j -= 1;
            }
            found.items[j] = item;
        }
        found
    }

    /// [`sliding_attackers`] under `occupied`, appending those not yet listed,
    /// in its order.
    ///
    /// Without a magic table the rays are ray-cast. This returned early instead,
    /// so SEE saw no rook, bishop, dragon, horse or lance at all whenever
    /// `magic_table.bin` was missing (a PC build from a tarball without the
    /// gitignored file searched different trees from the Mac; S41, 2026-11-12).
    fn add_sliders(&mut self, board: &BitboardBoard, to: Position, occupied: crate::types::Bitboard) {
        let to_idx = to.to_index();
        #[cfg(test)]
        let magic = if FORCE_RAYCAST.with(std::cell::Cell::get) { None } else { board.magic_table_ref() };
        #[cfg(not(test))]
        let magic = board.magic_table_ref();
        let (rook_rays, bishop_rays) = match magic {
            Some(magic) => (
                magic.get_attacks(to_idx, PieceType::Rook, occupied).to_u128(),
                magic.get_attacks(to_idx, PieceType::Bishop, occupied).to_u128(),
            ),
            None => (
                crate::bitboards::raycast_slider_attacks(to_idx, PieceType::Rook, occupied).to_u128(),
                crate::bitboards::raycast_slider_attacks(to_idx, PieceType::Bishop, occupied).to_u128(),
            ),
        };
        let pieces = board.get_pieces();
        let unseen = occupied.to_u128() & !self.seen;
        let bb = |player: Player, pt: PieceType| pieces[player as usize][pt.as_index()].to_u128();
        let straight = |player: Player| {
            bb(player, PieceType::Rook) | bb(player, PieceType::PromotedRook) | bb(player, PieceType::Lance)
        };
        let diagonal =
            |player: Player| bb(player, PieceType::Bishop) | bb(player, PieceType::PromotedBishop);
        let any = ((straight(Player::Black) | straight(Player::White)) & rook_rays
            | (diagonal(Player::Black) | diagonal(Player::White)) & bishop_rays)
            & unseen;
        if any == 0 {
            return;
        }
        for player in [Player::Black, Player::White] {
            for pt in [
                PieceType::Rook,
                PieceType::PromotedRook,
                PieceType::Bishop,
                PieceType::PromotedBishop,
                PieceType::Lance,
            ] {
                let rays = match pt {
                    PieceType::Bishop | PieceType::PromotedBishop => bishop_rays,
                    _ => rook_rays,
                };
                let mut v = bb(player, pt) & rays & any;
                while v != 0 {
                    let sq = v.trailing_zeros() as u8;
                    v &= v - 1;
                    let pos = Position::from_index(sq);
                    if pt == PieceType::Lance {
                        if pos.col != to.col {
                            continue;
                        }
                        let forward =
                            if player == Player::Black { pos.row > to.row } else { pos.row < to.row };
                        if !forward {
                            continue;
                        }
                    }
                    self.push((pos, Piece::new(pt, player)));
                }
            }
        }
    }
}

/// Slider ray attacks from square index `to` under `occupied`, as [`AttackList::add_sliders`]
/// computes them: from the magic table, ray-cast without one.
#[inline]
fn slider_rays(board: &BitboardBoard, to: u8, pt: PieceType, occupied: crate::types::Bitboard) -> u128 {
    #[cfg(test)]
    let magic = if FORCE_RAYCAST.with(std::cell::Cell::get) { None } else { board.magic_table_ref() };
    #[cfg(not(test))]
    let magic = board.magic_table_ref();
    match magic {
        Some(magic) => magic.get_attacks(to, pt, occupied).to_u128(),
        None => crate::bitboards::raycast_slider_attacks(to, pt, occupied).to_u128(),
    }
}

/// Squares, in the order they were found, of one side's pieces of one slider type that
/// attack the exchange square (at most four of a type exist).
#[derive(Clone, Copy, Default)]
struct SqQueue {
    sq: [u8; 4],
    head: u8,
    len: u8,
}

impl SqQueue {
    #[inline]
    fn push(&mut self, sq: u8) {
        if (self.len as usize) < self.sq.len() {
            self.sq[self.len as usize] = sq;
            self.len += 1;
        }
    }

    #[inline]
    fn front(&self) -> Option<u8> {
        (self.head < self.len).then(|| self.sq[self.head as usize])
    }

    /// Removes `sq` wherever it stands (the moving piece), keeping the order of the rest.
    fn remove(&mut self, sq: u8) {
        let (h, n) = (self.head as usize, self.len as usize);
        if let Some(i) = self.sq[h..n].iter().position(|&x| x == sq) {
            self.sq.copy_within(h + i + 1..n, h + i);
            self.len -= 1;
        }
    }
}

/// Slider types in [`Exchange::sliders`], and which of them move along ranks and files.
const SLIDERS: [PieceType; 5] =
    [PieceType::Lance, PieceType::Bishop, PieceType::Rook, PieceType::PromotedBishop, PieceType::PromotedRook];
const SLIDER_ROOK_LIKE: [bool; 5] = [true, false, true, false, true];

/// The attackers of one square for the exchange, as bitboards and small queues (S45).
///
/// Picks exactly the piece [`AttackList`] picks: the least valuable of the side to move,
/// ties broken by the list's order. Step attackers (pawn, knight, silver, the five
/// gold-movers, king) never change during an exchange; among equal values the list holds
/// them by type (gold, +P, +L, +N, +S) and then by square, which is the lowest set bit
/// type by type. Sliders can appear later (x-rays) and the list appends them, so each
/// side and slider type keeps its squares in the order found. The slider types all have
/// different values from each other and from the step types, so only the order within a
/// type matters.
#[derive(Clone, Copy)]
struct Exchange {
    /// Step attackers of both sides still to capture.
    steps: u128,
    sliders: [[SqQueue; 5]; 2],
    /// Squares ever listed, plus the target (as [`AttackList::seen`]).
    seen: u128,
}

impl Exchange {
    fn attackers_of(square: Position, board: &BitboardBoard) -> Self {
        let pieces = board.get_pieces();
        let to = square.to_index();
        let bb = |player: Player, pt: PieceType| pieces[player as usize][pt.as_index()].to_u128();
        let mut x = Self { steps: 0, sliders: [[SqQueue::default(); 5]; 2], seen: 1u128 << to };
        let occupied = board.get_occupied_bitboard();
        for player in [Player::Black, Player::White] {
            // Step attacks, looked up from the target with the other colour's patterns (as
            // `AttackList::attackers_of`); the promoted minors move as a gold.
            let pat = |pt: PieceType| board.get_attack_pattern_precomputed(square, pt, player.opposite()).to_u128();
            let gold_movers = bb(player, PieceType::Gold)
                | bb(player, PieceType::PromotedPawn)
                | bb(player, PieceType::PromotedLance)
                | bb(player, PieceType::PromotedKnight)
                | bb(player, PieceType::PromotedSilver);
            let adjacent = board.get_attack_pattern_precomputed(square, PieceType::King, player).to_u128();
            x.steps |= pat(PieceType::Pawn) & bb(player, PieceType::Pawn)
                | pat(PieceType::Knight) & bb(player, PieceType::Knight)
                | pat(PieceType::Silver) & bb(player, PieceType::Silver)
                | pat(PieceType::Gold) & gold_movers
                | adjacent & bb(player, PieceType::King);
            // Horse and dragon one step away come first, as the list finds them.
            let q = &mut x.sliders[player as usize];
            for k in [3, 4] {
                let mut v = adjacent & bb(player, SLIDERS[k]);
                while v != 0 {
                    q[k].push(v.trailing_zeros() as u8);
                    x.seen |= v & v.wrapping_neg();
                    v &= v - 1;
                }
            }
        }
        x.seen |= x.steps;
        let t = square.to_index();
        x.add_sliders(
            board,
            square,
            occupied.to_u128(),
            Some(slider_rays(board, t, PieceType::Rook, occupied)),
            Some(slider_rays(board, t, PieceType::Bishop, occupied)),
        );
        x
    }

    /// Appends the sliders newly visible on `to` (not yet seen) under `occupied`, by square
    /// within each side and type, for the ray kinds given.
    #[inline]
    fn add_sliders(
        &mut self,
        board: &BitboardBoard,
        to: Position,
        occupied: u128,
        rook_rays: Option<u128>,
        bishop_rays: Option<u128>,
    ) {
        let pieces = board.get_pieces();
        let unseen = occupied & !self.seen;
        for player in [Player::Black, Player::White] {
            for (k, pt) in SLIDERS.iter().enumerate() {
                let Some(rays) = (if SLIDER_ROOK_LIKE[k] { rook_rays } else { bishop_rays }) else {
                    continue;
                };
                let mut v = pieces[player as usize][pt.as_index()].to_u128() & rays & unseen;
                while v != 0 {
                    let sq = v.trailing_zeros() as u8;
                    v &= v - 1;
                    if *pt == PieceType::Lance {
                        // Same file, and in front of the target from its own side.
                        let pos = Position::from_index(sq);
                        if pos.col != to.col {
                            continue;
                        }
                        let forward = if player == Player::Black { pos.row > to.row } else { pos.row < to.row };
                        if !forward {
                            continue;
                        }
                    }
                    self.sliders[player as usize][k].push(sq);
                    self.seen |= 1u128 << sq;
                }
            }
        }
    }

    /// After the piece on `from` (on a line through `to`) left `occupied`: a square on a
    /// rank or file through `to` can open only rook-like rays, one on a diagonal only
    /// bishop-like ones (no square but `to` is on both), and every slider visible before
    /// is already listed.
    #[inline]
    fn uncover(&mut self, board: &BitboardBoard, to: Position, from: Position, occupied: crate::types::Bitboard) {
        let t = to.to_index();
        if from.row == to.row || from.col == to.col {
            let rays = slider_rays(board, t, PieceType::Rook, occupied);
            self.add_sliders(board, to, occupied.to_u128(), Some(rays), None);
        } else {
            let rays = slider_rays(board, t, PieceType::Bishop, occupied);
            self.add_sliders(board, to, occupied.to_u128(), None, Some(rays));
        }
    }

    /// Removes the piece on `sq` (the mover) wherever it is listed.
    fn remove_square(&mut self, sq: u8) {
        self.steps &= !(1u128 << sq);
        for side in &mut self.sliders {
            for q in side.iter_mut() {
                q.remove(sq);
            }
        }
    }

    /// The least valuable remaining attacker of `side` (ties in list order), taken out.
    #[inline]
    fn pop_least(&mut self, board: &BitboardBoard, side: Player) -> Option<(u8, PieceType)> {
        let own = &board.get_pieces()[side as usize];
        let steps = self.steps;
        let step = |pt: PieceType| {
            let v = steps & own[pt.as_index()].to_u128();
            (v != 0).then(|| v.trailing_zeros() as u8)
        };
        // By value: pawn 100, lance 300, knight 320, silver 450, the gold-movers 500,
        // bishop 800, rook 1000, horse 1200, dragon 1300, king.
        const GOLD_MOVERS: [PieceType; 5] = [
            PieceType::Gold,
            PieceType::PromotedPawn,
            PieceType::PromotedLance,
            PieceType::PromotedKnight,
            PieceType::PromotedSilver,
        ];
        let q = &mut self.sliders[side as usize];
        let (sq, pt, slot) = if let Some(s) = step(PieceType::Pawn) {
            (s, PieceType::Pawn, None)
        } else if let Some(s) = q[0].front() {
            (s, PieceType::Lance, Some(0))
        } else if let Some(s) = step(PieceType::Knight) {
            (s, PieceType::Knight, None)
        } else if let Some(s) = step(PieceType::Silver) {
            (s, PieceType::Silver, None)
        } else if let Some((s, pt)) = GOLD_MOVERS.into_iter().find_map(|pt| step(pt).map(|s| (s, pt))) {
            (s, pt, None)
        } else if let Some((k, s)) = (1..5).find_map(|k| q[k].front().map(|s| (k, s))) {
            (s, SLIDERS[k], Some(k))
        } else {
            let s = step(PieceType::King)?;
            (s, PieceType::King, None)
        };
        match slot {
            Some(k) => q[k].head += 1,
            None => self.steps &= !(1u128 << sq),
        }
        Some((sq, pt))
    }
}

/// The swap-list exchange on `move_.to`, starting from `first_gain` -- the
/// value of what the move itself captures, 0 for a quiet move.
///
/// The same computation as before (`swap_value_reference` in the tests),
/// without heap allocation: SEE runs for every attacked quiet move in move
/// ordering.
fn swap_value(move_: &Move, board: &BitboardBoard, first_gain: i32) -> i32 {
    exchange_from(Exchange::attackers_of(move_.to, board), move_, board, first_gain)
}

/// Quiet-move SEE ([`see_quiet`]) for many moves on one board. The attackers
/// of a square under the board's occupancy do not depend on the move, so each
/// target square's list is built once and shared by every move onto it --
/// drops of different pieces, and pieces converging on one square.
pub struct QuietSee {
    index: [u8; 81],
    lists: Vec<Exchange>,
}

impl Default for QuietSee {
    fn default() -> Self {
        Self { index: [u8::MAX; 81], lists: Vec::new() }
    }
}

impl QuietSee {
    /// [`see_quiet`] of `move_` on `board`, which must be the same board on
    /// every call.
    pub fn see_quiet(&mut self, move_: &Move, board: &BitboardBoard) -> i32 {
        let t = move_.to.to_index() as usize;
        if self.index[t] == u8::MAX {
            self.index[t] = self.lists.len() as u8;
            self.lists.push(Exchange::attackers_of(move_.to, board));
        }
        exchange_from(self.lists[self.index[t] as usize], move_, board, 0)
    }
}

/// Whether `a` lies on a rank, file or diagonal through `b`: the only squares whose
/// emptying can open a rook, bishop, dragon, horse or lance ray onto `b`.
#[inline]
fn on_line(a: Position, b: Position) -> bool {
    let (dr, dc) = (i32::from(a.row) - i32::from(b.row), i32::from(a.col) - i32::from(b.col));
    dr == 0 || dc == 0 || dr.abs() == dc.abs()
}

/// [`swap_value`] given the attackers of `move_.to` under the board's occupancy: the
/// list version [`exchange_from`] replaced (S45), kept as its test reference.
#[cfg(test)]
fn swap_value_from(
    mut pieces: AttackList,
    move_: &Move,
    board: &BitboardBoard,
    first_gain: i32,
) -> i32 {
    let to = move_.to;
    let mover = move_.player;

    let mut on_square = match move_.from.and_then(|f| board.get_piece(f)) {
        Some(p) => p.piece_type.base_value(),
        None => move_.piece_type.base_value(), // drop
    };

    let mut occupied = board.get_occupied_bitboard();
    if let Some(from) = move_.from {
        occupied &= !crate::types::Bitboard::from_u128(1u128 << from.to_index());
    }
    if let Some(from) = move_.from {
        if let Some(i) = pieces.as_slice().iter().position(|(p, _)| *p == from) {
            pieces.remove(i);
        }
        // Lifting the mover can uncover a slider only through a square on a line through the
        // target. A drop vacates nothing: the occupancy is the one `attackers_of` used.
        if on_line(from, to) {
            pieces.add_sliders(board, to, occupied);
        }
    }

    let mut gain = [0i32; 40];
    gain[0] = first_gain;
    let mut d = 0usize;
    let mut side = mover.opposite();

    while d + 1 < gain.len() {
        d += 1;
        gain[d] = on_square - gain[d - 1];
        if (-gain[d - 1]).max(gain[d]) < 0 {
            break;
        }
        let Some(idx) = pieces
            .as_slice()
            .iter()
            .enumerate()
            .filter(|(_, (_, p))| p.player == side)
            .min_by_key(|(_, (_, p))| p.piece_type.base_value())
            .map(|(i, _)| i)
        else {
            break;
        };
        let (used_pos, used_piece) = pieces.remove(idx);
        on_square = used_piece.piece_type.base_value();
        occupied &= !crate::types::Bitboard::from_u128(1u128 << used_pos.to_index());
        // A piece off every line through the target (a knight always) uncovers nothing.
        if on_line(used_pos, to) {
            pieces.add_sliders(board, to, occupied);
        }
        side = side.opposite();
    }

    while d > 1 {
        d -= 1;
        gain[d - 1] = -((-gain[d - 1]).max(gain[d]));
    }
    gain[0]
}

/// [`swap_value_from`] on an [`Exchange`]: the same exchange, the same pieces picked.
fn exchange_from(mut x: Exchange, move_: &Move, board: &BitboardBoard, first_gain: i32) -> i32 {
    let to = move_.to;
    let mover = move_.player;
    let mut on_square = match move_.from.and_then(|f| board.get_piece(f)) {
        Some(p) => p.piece_type.base_value(),
        None => move_.piece_type.base_value(), // drop
    };
    let mut occupied = board.get_occupied_bitboard();
    if let Some(from) = move_.from {
        occupied &= !crate::types::Bitboard::from_u128(1u128 << from.to_index());
        x.remove_square(from.to_index());
        // A drop vacates nothing: the occupancy is the one `attackers_of` used.
        if on_line(from, to) {
            x.uncover(board, to, from, occupied);
        }
    }
    let mut gain = [0i32; 40];
    gain[0] = first_gain;
    let mut d = 0usize;
    let mut side = mover.opposite();
    while d + 1 < gain.len() {
        d += 1;
        gain[d] = on_square - gain[d - 1];
        if (-gain[d - 1]).max(gain[d]) < 0 {
            break;
        }
        let Some((sq, pt)) = x.pop_least(board, side) else {
            break;
        };
        on_square = pt.base_value();
        occupied &= !crate::types::Bitboard::from_u128(1u128 << sq);
        let used = Position::from_index(sq);
        // A piece off every line through the target (a knight always) uncovers nothing.
        if on_line(used, to) {
            x.uncover(board, to, used, occupied);
        }
        side = side.opposite();
    }
    while d > 1 {
        d -= 1;
        gain[d - 1] = -((-gain[d - 1]).max(gain[d]));
    }
    gain[0]
}

#[cfg(test)]
fn find_attackers_defenders_reference(square: Position, board: &BitboardBoard) -> Vec<(Position, Piece)> {
    // Built on the board's own attack tables -- the ones `is_square_attacked_by`
    // uses and the board oracle tests cover. The hand-written version this
    // replaces had pawns, lances and knights moving the wrong way for both
    // colours and gave golds, silvers and promoted minors a king's eight
    // squares, so SEE saw attackers that were not there and missed ones that
    // were.
    use crate::bitboards::integration::GlobalOptimizer;
    let pieces = board.get_pieces();
    let mut found: Vec<(Position, Piece)> = Vec::new();
    let push_all = |mut bb: crate::types::Bitboard, player: Player, pt: PieceType, found: &mut Vec<(Position, Piece)>| {
        while let Some(sq) = GlobalOptimizer::bit_scan_forward(bb) {
            bb &= crate::types::Bitboard::from_u128(bb.to_u128() - 1);
            let pos = Position::from_index(sq);
            if pos != square && !found.iter().any(|(p, _)| *p == pos) {
                found.push((pos, Piece::new(pt, player)));
            }
        }
    };
    for player in [Player::Black, Player::White] {
        let own = &pieces[player as usize];
        // Step attacks, looked up from the target: pieces are point-symmetric
        // between the colours, so `player`'s pieces that attack `square` sit on
        // the squares a same-type piece of the other colour attacks from it.
        for pt in [
            PieceType::Pawn,
            PieceType::Knight,
            PieceType::Silver,
            PieceType::Gold,
            PieceType::PromotedPawn,
            PieceType::PromotedLance,
            PieceType::PromotedKnight,
            PieceType::PromotedSilver,
        ] {
            let bb = board.get_attack_pattern_precomputed(square, pt, player.opposite()) & own[pt.as_index()];
            push_all(bb, player, pt, &mut found);
        }
        // The king, and the one-step moves of dragon and horse.
        let adjacent = board.get_attack_pattern_precomputed(square, PieceType::King, player);
        for pt in [PieceType::King, PieceType::PromotedRook, PieceType::PromotedBishop] {
            push_all(adjacent & own[pt.as_index()], player, pt, &mut found);
        }
    }
    // Sliders (rook, bishop, dragon, horse rays, lance) under the current occupancy.
    for (pos, piece) in sliding_attackers(board, square, board.get_occupied_bitboard()) {
        if pos != square && !found.iter().any(|(p, _)| *p == pos) {
            found.push((pos, piece));
        }
    }

    // Sort by piece value (ascending) - least valuable first for SEE
    found.sort_by_key(|(_, p)| p.piece_type.base_value());
    found
}

#[cfg(test)]
/// The allocating original of [`swap_value`], kept as its test reference.
///
/// The swap-list exchange on `move_.to`, starting from `first_gain` -- the
/// value of what the move itself captures, 0 for a quiet move.
fn swap_value_reference(move_: &Move, board: &BitboardBoard, first_gain: i32) -> i32 {
    let to = move_.to;
    let mover = move_.player;

    // Value of whatever now stands on the target square: it is what the
    // opponent wins if they recapture.
    let mut on_square = match move_.from.and_then(|f| board.get_piece(f)) {
        Some(p) => p.piece_type.base_value(),
        None => move_.piece_type.base_value(), // drop
    };

    // Everything else bearing on the square. The piece that just moved is no
    // longer among them, and its square is no longer occupied — which may
    // already have opened a line onto the target.
    let mut occupied = board.get_occupied_bitboard();
    if let Some(from) = move_.from {
        occupied &= !crate::types::Bitboard::from_u128(1u128 << from.to_index());
    }
    let mut pieces: Vec<(Position, Piece)> = find_attackers_defenders_reference(to, board)
        .into_iter()
        .filter(|(pos, _)| Some(*pos) != move_.from)
        .collect();
    // Sliders revealed by the mover vacating `from`.
    for (pos, piece) in sliding_attackers(board, to, occupied) {
        if pos != to && !pieces.iter().any(|(p, _)| *p == pos) {
            pieces.push((pos, piece));
        }
    }

    // The standard swap list. `gain[d]` is the material balance for the side
    // to move at depth d if the exchange runs that far; folding it back with
    // the negamax maximum lets either side stand pat when continuing would
    // lose, which is what makes SEE an exchange evaluation rather than a sum.
    //
    // What this replaces did two things wrong: it subtracted the moving
    // piece's value up front, though that piece is only lost if it is
    // recaptured, and on each recapture it subtracted the value of the piece
    // *doing* the capturing rather than the piece being taken. An even
    // pawn-for-pawn trade defended by a rook came out at -990.
    let mut gain = [0i32; 40];
    gain[0] = first_gain;
    let mut d = 0usize;
    let mut side = mover.opposite();

    while d + 1 < gain.len() {
        d += 1;
        gain[d] = on_square - gain[d - 1];
        // Neither side will enter an exchange that loses by every measure.
        if (-gain[d - 1]).max(gain[d]) < 0 {
            break;
        }
        let Some(idx) = pieces
            .iter()
            .enumerate()
            .filter(|(_, (_, p))| p.player == side)
            .min_by_key(|(_, (_, p))| p.piece_type.base_value())
            .map(|(i, _)| i)
        else {
            break;
        };
        let (used_pos, used_piece) = pieces.remove(idx);
        on_square = used_piece.piece_type.base_value();
        // That piece has left its square, which may reveal a slider behind it.
        occupied &= !crate::types::Bitboard::from_u128(1u128 << used_pos.to_index());
        for (pos, piece) in sliding_attackers(board, to, occupied) {
            if pos != to && !pieces.iter().any(|(p, _)| *p == pos) {
                pieces.push((pos, piece));
            }
        }
        side = side.opposite();
    }

    while d > 1 {
        d -= 1;
        gain[d - 1] = -((-gain[d - 1]).max(gain[d]));
    }
    gain[0]
}

/// Score a move using Static Exchange Evaluation (SEE)
///
/// SEE evaluates the material gain/loss from a sequence of captures
/// starting with the given move. This provides a more accurate assessment
/// of capture moves than simple piece values.
///
/// # Arguments
/// * `move_` - The move to score
/// * `board` - The current board position
/// * `see_weight` - Weight for SEE scores
///
/// # Returns
/// The SEE score for the move, scaled by the weight
pub fn score_see_move(move_: &Move, board: &BitboardBoard, see_weight: i32) -> SEEResult<i32> {
    if !move_.is_capture {
        return Ok(0);
    }

    let see_value = calculate_see_internal(move_, board);
    let see_score = (see_value * see_weight) / 1000;

    Ok(see_score)
}

/// SEE cache entry with metadata for eviction policies
/// Task 7.0: Enhanced with LRU tracking
#[derive(Debug, Clone)]
pub struct SEECacheEntry {
    /// Cached SEE value
    pub value: i32,
    /// Last access timestamp (for LRU tracking)
    pub last_access: u64,
    /// Number of times this entry was accessed
    pub access_count: u64,
    /// Absolute value of SEE (for value-based eviction)
    pub see_abs_value: i32,
}

/// SEE cache manager
///
/// Manages caching of SEE calculation results for performance optimization.
/// Task 7.0: Enhanced with advanced eviction policies (FIFO, LRU, Value-Based)
#[derive(Debug, Clone)]
pub struct SEECache {
    /// SEE cache: maps (from_square, to_square) -> cache entry
    cache: HashMap<(Position, Position), SEECacheEntry>,
    /// Maximum cache size
    max_size: usize,
    /// LRU access counter (incremented on each access)
    lru_access_counter: u64,
}

impl SEECache {
    /// Create a new SEE cache
    pub fn new(max_size: usize) -> Self {
        Self { cache: HashMap::new(), max_size, lru_access_counter: 0 }
    }

    /// Get a cached SEE value
    /// Task 7.0: Updates LRU tracking on access
    pub fn get(&mut self, from: Position, to: Position) -> Option<i32> {
        if let Some(entry) = self.cache.get_mut(&(from, to)) {
            self.lru_access_counter += 1;
            entry.last_access = self.lru_access_counter;
            entry.access_count += 1;
            Some(entry.value)
        } else {
            None
        }
    }

    /// Cache a SEE value
    /// Task 7.0: Evicts least valuable entry when cache is full
    pub fn insert(&mut self, from: Position, to: Position, value: i32) -> bool {
        let key = (from, to);

        // If entry already exists, update it
        if let Some(entry) = self.cache.get_mut(&key) {
            self.lru_access_counter += 1;
            entry.value = value;
            entry.last_access = self.lru_access_counter;
            entry.access_count += 1;
            entry.see_abs_value = value.abs();
            return false; // No eviction
        }

        // If cache has room, just insert
        if self.cache.len() < self.max_size {
            self.lru_access_counter += 1;
            let entry = SEECacheEntry {
                value,
                last_access: self.lru_access_counter,
                access_count: 1,
                see_abs_value: value.abs(),
            };
            self.cache.insert(key, entry);
            return false; // No eviction
        }

        // Cache is full - use LRU eviction with value-preference
        // Prefer evicting entries with low absolute SEE values and low access counts
        if let Some(evict_key) = self.select_eviction_candidate() {
            self.cache.remove(&evict_key);

            self.lru_access_counter += 1;
            let entry = SEECacheEntry {
                value,
                last_access: self.lru_access_counter,
                access_count: 1,
                see_abs_value: value.abs(),
            };
            self.cache.insert(key, entry);
            return true; // Eviction occurred
        }

        false
    }

    /// Select a cache entry for eviction
    /// Task 7.0: Hybrid eviction policy combining LRU and value-based eviction
    /// Prefers evicting entries with low absolute SEE values and low recent
    /// access
    fn select_eviction_candidate(&self) -> Option<(Position, Position)> {
        if self.cache.is_empty() {
            return None;
        }

        // Score each entry: lower score = more likely to evict
        // Score = (access_age_weight * normalized_age) + (value_weight *
        // normalized_inverse_value)
        let access_age_weight = 0.6;
        let value_weight = 0.4;

        // Find min/max for normalization
        let max_access = self.cache.values().map(|e| e.last_access).max().unwrap_or(1);
        let min_access = self.cache.values().map(|e| e.last_access).min().unwrap_or(1);
        let access_range = (max_access - min_access).max(1) as f32;

        let max_value = self.cache.values().map(|e| e.see_abs_value).max().unwrap_or(1);
        let min_value = self.cache.values().map(|e| e.see_abs_value).min().unwrap_or(0);
        let value_range = (max_value - min_value).max(1) as f32;

        let mut evict_key = None;
        let mut evict_score = f32::MAX;

        for (key, entry) in &self.cache {
            // Age score: normalized to 0.0 (oldest) .. 1.0 (newest)
            // Invert so older = higher score (more likely to evict)
            let age_score = if access_range > 0.0 {
                1.0 - ((entry.last_access - min_access) as f32 / access_range)
            } else {
                0.5
            };

            // Value score: normalized to 0.0 (lowest value) .. 1.0 (highest value)
            // Invert so lower value = higher score (more likely to evict)
            let value_score = if value_range > 0.0 {
                1.0 - ((entry.see_abs_value - min_value) as f32 / value_range)
            } else {
                0.5
            };

            // Combined score: higher = more likely to evict
            let combined_score = access_age_weight * age_score + value_weight * value_score;

            if combined_score > evict_score {
                evict_score = combined_score;
                evict_key = Some(*key);
            }
        }

        evict_key
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
        self.lru_access_counter = 0;
    }

    /// Get cache size
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Check if cache is empty
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Check if cache is full
    pub fn is_full(&self) -> bool {
        self.cache.len() >= self.max_size
    }

    /// Get cache utilization as a percentage
    pub fn utilization(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.cache.len() as f64 / self.max_size as f64) * 100.0
        }
    }

    /// Get memory usage estimate for cache
    pub fn memory_bytes(&self) -> usize {
        self.cache.len()
            * (std::mem::size_of::<(Position, Position)>() + std::mem::size_of::<SEECacheEntry>())
    }

    /// Get maximum cache size
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Set maximum cache size
    /// If new size is smaller than current cache size, evicts entries
    pub fn set_max_size(&mut self, new_max_size: usize) {
        self.max_size = new_max_size;

        // Evict entries if cache exceeds new max size
        while self.cache.len() > self.max_size {
            if let Some(evict_key) = self.select_eviction_candidate() {
                self.cache.remove(&evict_key);
            } else {
                break;
            }
        }
    }

    /// Get cache statistics
    pub fn get_stats(&self) -> SEECacheStats {
        let total_access_count: u64 = self.cache.values().map(|e| e.access_count).sum();
        let avg_access_count = if !self.cache.is_empty() {
            total_access_count as f64 / self.cache.len() as f64
        } else {
            0.0
        };

        SEECacheStats {
            size: self.cache.len(),
            max_size: self.max_size,
            utilization: self.utilization(),
            total_accesses: total_access_count,
            avg_accesses_per_entry: avg_access_count,
            memory_bytes: self.memory_bytes(),
        }
    }
}

/// SEE cache statistics
#[derive(Debug, Clone)]
pub struct SEECacheStats {
    /// Current cache size
    pub size: usize,
    /// Maximum cache size
    pub max_size: usize,
    /// Cache utilization percentage
    pub utilization: f64,
    /// Total accesses across all entries
    pub total_accesses: u64,
    /// Average accesses per cache entry
    pub avg_accesses_per_entry: f64,
    /// Memory usage in bytes
    pub memory_bytes: usize,
}

impl Default for SEECache {
    fn default() -> Self {
        Self::new(5000) // Default max size (increased from 1000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::moves::MoveGenerator;
    use crate::types::board::CapturedPieces;
    use rand::{seq::SliceRandom, SeedableRng};

    /// The stack-based exchange must give exactly the allocating original's
    /// values, and the attacker lists must match entry for entry, along random
    /// games (captures, drops, promotions, x-rays).
    #[test]
    fn swap_value_matches_reference() {
        let mg = MoveGenerator::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(51);
        let (mut compared, mut nonzero) = (0u64, 0u64);
        for _ in 0..40 {
            let mut board = BitboardBoard::new();
            let mut captured = CapturedPieces::new();
            let mut player = Player::Black;
            for _ in 0..160 {
                for idx in 0..81u8 {
                    let sq = Position::from_index(idx);
                    assert_eq!(find_attackers_defenders(sq, &board), find_attackers_defenders_reference(sq, &board));
                }
                for m in mg.generate_legal_moves(&board, player, &captured) {
                    let first = m.captured_piece.map_or(0, |p| p.piece_type.base_value());
                    for g in [0, first] {
                        let got = swap_value(&m, &board, g);
                        assert_eq!(got, swap_value_reference(&m, &board, g), "{}", m.to_usi_string());
                        compared += 1;
                        nonzero += (got != g) as u64;
                    }
                }
                let legal = mg.generate_legal_moves(&board, player, &captured);
                let Some(m) = legal.choose(&mut rng) else { break };
                board.make_move_with_hand(m, &mut captured);
                player = player.opposite();
            }
        }
        assert!(nonzero > 1000, "{compared} {nonzero}");
    }

    /// The promoted minors move as a gold, for both colours on every square
    /// (`Exchange::attackers_of` looks up the gold pattern for all five).
    #[test]
    fn promoted_minors_move_as_gold() {
        let board = BitboardBoard::new();
        for idx in 0..81u8 {
            let sq = Position::from_index(idx);
            for player in [Player::Black, Player::White] {
                let gold = board.get_attack_pattern_precomputed(sq, PieceType::Gold, player);
                for pt in [
                    PieceType::PromotedPawn,
                    PieceType::PromotedLance,
                    PieceType::PromotedKnight,
                    PieceType::PromotedSilver,
                ] {
                    assert_eq!(board.get_attack_pattern_precomputed(sq, pt, player), gold, "{pt:?} {idx}");
                }
            }
        }
    }

    /// The bitboard exchange (S45) picks the pieces the list version picked: equal values
    /// for every legal move, quiet and capture, along random games, with and without the
    /// magic table. Counts positions where equal-valued attackers of one side compete, so
    /// the tie order is exercised.
    #[test]
    fn exchange_matches_attack_list() {
        let mg = MoveGenerator::new();
        let mut rng = rand::rngs::StdRng::seed_from_u64(4545);
        let (mut compared, mut nonzero, mut ties) = (0u64, 0u64, 0u64);
        for _ in 0..60 {
            let mut board = BitboardBoard::new();
            let mut captured = CapturedPieces::new();
            let mut player = Player::Black;
            for _ in 0..200 {
                for m in mg.generate_legal_moves(&board, player, &captured) {
                    let first = m.captured_piece.map_or(0, |p| p.piece_type.base_value());
                    let list = AttackList::attackers_of(m.to, &board);
                    let mut vals: Vec<(Player, i32)> =
                        list.as_slice().iter().map(|(_, p)| (p.player, p.piece_type.base_value())).collect();
                    vals.sort_unstable_by_key(|&(pl, v)| (pl as u8, v));
                    ties += vals.windows(2).any(|w| w[0] == w[1]) as u64;
                    for raycast in [false, true] {
                        FORCE_RAYCAST.with(|f| f.set(raycast));
                        let list = AttackList::attackers_of(m.to, &board);
                        let x = Exchange::attackers_of(m.to, &board);
                        for g in [0, first] {
                            let want = swap_value_from(list, &m, &board, g);
                            let got = exchange_from(x, &m, &board, g);
                            assert_eq!(got, want, "{} raycast {raycast}", m.to_usi_string());
                            compared += 1;
                            nonzero += (got != g) as u64;
                        }
                        FORCE_RAYCAST.with(|f| f.set(false));
                    }
                }
                let legal = mg.generate_legal_moves(&board, player, &captured);
                let Some(m) = legal.choose(&mut rng) else { break };
                board.make_move_with_hand(m, &mut captured);
                player = player.opposite();
            }
        }
        assert!(nonzero > 10_000 && ties > 10_000, "{compared} {nonzero} {ties}");
    }

    /// Without a magic table, SEE must give the values it gives with one: its
    /// slider rays are ray-cast instead. It used to skip sliders entirely when
    /// the table was missing, so this fails on that version (`differs` from the
    /// table run, and `slider_moves` would be counted on one side only).
    #[test]
    fn see_without_magic_table_matches_table() {
        let mg = MoveGenerator::new();
        let board0 = BitboardBoard::new();
        assert!(board0.magic_table_ref().is_some(), "needs resources/magic_tables/magic_table.bin");
        let mut rng = rand::rngs::StdRng::seed_from_u64(41);
        let (mut compared, mut slider_moves) = (0u64, 0u64);
        for _ in 0..30 {
            let mut board = BitboardBoard::new();
            let mut captured = CapturedPieces::new();
            let mut player = Player::Black;
            for _ in 0..140 {
                for m in mg.generate_legal_moves(&board, player, &captured) {
                    let with_table = (calculate_see_internal(&m, &board), see_quiet(&m, &board));
                    let sliders_seen = find_attackers_defenders(m.to, &board)
                        .iter()
                        .any(|(_, p)| matches!(p.piece_type, PieceType::Rook | PieceType::Bishop
                            | PieceType::PromotedRook | PieceType::PromotedBishop | PieceType::Lance));
                    FORCE_RAYCAST.with(|f| f.set(true));
                    let without = (calculate_see_internal(&m, &board), see_quiet(&m, &board));
                    let sliders_without = find_attackers_defenders(m.to, &board)
                        .iter()
                        .any(|(_, p)| matches!(p.piece_type, PieceType::Rook | PieceType::Bishop
                            | PieceType::PromotedRook | PieceType::PromotedBishop | PieceType::Lance));
                    FORCE_RAYCAST.with(|f| f.set(false));
                    assert_eq!(with_table, without, "{}", m.to_usi_string());
                    assert_eq!(sliders_seen, sliders_without, "{}", m.to_usi_string());
                    compared += 1;
                    slider_moves += sliders_seen as u64;
                }
                let legal = mg.generate_legal_moves(&board, player, &captured);
                let Some(m) = legal.choose(&mut rng) else { break };
                board.make_move_with_hand(m, &mut captured);
                player = player.opposite();
            }
        }
        assert!(slider_moves > 10_000, "{compared} {slider_moves}");
    }
}
