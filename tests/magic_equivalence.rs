//! Magic-bitboard attacks must equal ray-cast attacks.
//!
//! No `BitboardBoard` constructor attaches a magic table, so every slider attack
//! in the engine has been computed by the ray-cast fallback. Before magic tables
//! can be switched on, they have to produce exactly the attacks the fallback
//! produces — otherwise enabling them silently changes move generation and check
//! detection.

use shogi_engine::bitboards::magic::attack_generator::AttackGenerator;
use shogi_engine::bitboards::magic::magic_table::MagicTable;
use shogi_engine::types::core::PieceType;
use shogi_engine::types::Bitboard;

const SLIDERS: [PieceType; 4] = [
    PieceType::Rook,
    PieceType::Bishop,
    PieceType::PromotedRook,
    PieceType::PromotedBishop,
];

fn occupancies() -> Vec<u128> {
    let board_mask: u128 = (1u128 << 81) - 1;
    let mut state: u128 = 0x9E37_79B9_7F4A_7C15_F39C_C060_5CED_C834;
    let mut next = || {
        state = state
            .wrapping_mul(0x2360_ED05_1FC6_5DA4_4385_DF64_9FCC_F645)
            .wrapping_add(0x5851_F42D_4C95_7F2D_1405_7B7E_F767_814F);
        state
    };
    let mut out = vec![0, board_mask];
    for _ in 0..150 {
        out.push(next() & board_mask); // ~half the board occupied
        out.push(next() & next() & board_mask); // ~quarter
        out.push(next() & next() & next() & next() & board_mask); // sparse
    }
    out
}

#[test]
#[ignore = "MagicTable::new() currently fails after ~195 s with GenerationFailed { square: 4, \
            piece_type: Rook }, so the engine has never used magic bitboards. Run with \
            `--ignored` once generation is fixed; this is the gate for switching them on."]
fn magic_attacks_match_raycast_for_every_slider_square_and_occupancy() {
    let t0 = std::time::Instant::now();
    let table = MagicTable::new().expect("magic table generation");
    eprintln!("MagicTable::new() took {:.2?}", t0.elapsed());

    let mut raycast = AttackGenerator::new();
    let mut mismatches: std::collections::BTreeMap<String, usize> = Default::default();
    let mut example: std::collections::BTreeMap<String, String> = Default::default();
    let occs = occupancies();

    for &pt in SLIDERS.iter() {
        for sq in 0..81u8 {
            for &occ in &occs {
                let occupied = Bitboard::from_u128(occ);
                let magic = table.get_attacks(sq, pt, occupied);
                let reference = raycast.generate_attack_pattern(sq, pt, occupied);
                if magic != reference {
                    let key = format!("{:?}", pt);
                    *mismatches.entry(key.clone()).or_default() += 1;
                    example.entry(key).or_insert_with(|| {
                        format!(
                            "sq {} occ {:#x}: magic {:#x} raycast {:#x} (raycast-only {:#x}, magic-only {:#x})",
                            sq,
                            occ,
                            magic.to_u128(),
                            reference.to_u128(),
                            reference.to_u128() & !magic.to_u128(),
                            magic.to_u128() & !reference.to_u128(),
                        )
                    });
                }
            }
        }
    }

    let per_piece = 81 * occs.len();
    for (pt, n) in &mismatches {
        eprintln!("{:<15} {:>6}/{} mismatches   e.g. {}", pt, n, per_piece, example[pt]);
    }
    assert!(mismatches.is_empty(), "magic attacks disagree with ray-cast: {:?}", mismatches);
}
