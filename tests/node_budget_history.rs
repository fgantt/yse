//! A `go nodes` search must not depend on what the engine searched before.
//!
//! Nodes reach the global counter (`GLOBAL_SEARCH_NODES`, which `go nodes` and
//! `info nodes` read) in batches of 256. The last partial batch of one search
//! used to stay in the engine's statistics and count toward the next search's
//! budget, so the same position after `usinewgame` searched up to 255 fewer
//! nodes depending on the previous search, and fixed-node games between two
//! copies of one engine diverged (S41, 2026-11-12: 1 of 10 colour-swapped A/A
//! pairs identical; 10 of 10 with the fix).
//!
//! Its own test binary: the node counter is process-global.
use shogi_engine::bitboards::BitboardBoard;
use shogi_engine::search::search_engine::{lean_new_game, SearchEngine};
use shogi_engine::search::statistics::GLOBAL_SEARCH_NODES;
use shogi_engine::search::IterativeDeepening;
use shogi_engine::types::core::Move;
use std::sync::atomic::Ordering;

const P: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/2P6/PP1PPPPPP/1B5R1/LNSGKGSNL w - 2";
const Q: &str = "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1";

/// The result, the global node count, and the engine's own count of main-search
/// nodes in the iteration the budget stopped (where a shifted stop shows).
fn search(engine: &mut SearchEngine, sfen: &str, budget: u64) -> (Option<(Move, i32)>, u64, u64) {
    let (board, player, captured) = BitboardBoard::from_fen(sfen).expect("valid sfen");
    engine.set_node_limit(Some(budget));
    let mut id = IterativeDeepening::new(64, 600_000, None);
    let result = id.search(engine, &board, &captured, player);
    (result, GLOBAL_SEARCH_NODES.load(Ordering::Relaxed), engine.get_nodes_searched())
}

#[test]
fn a_node_limited_search_does_not_depend_on_the_previous_search() {
    lean_new_game();
    let mut fresh = SearchEngine::new(None, 16);
    let (want, want_nodes, want_last) = search(&mut fresh, P, 30_000);

    // A depth-limited search ends anywhere in a batch, so it leaves a sizeable
    // remainder (a node-limited one stops just after a flush: 0-3 nodes, still
    // enough to make fixed-node games diverge).
    let mut used = SearchEngine::new(None, 16);
    let (board, player, captured) = BitboardBoard::from_fen(Q).expect("valid sfen");
    let mut id = IterativeDeepening::new(5, 600_000, None);
    let _ = id.search(&mut used, &board, &captured, player);
    assert!(
        used.pending_global_nodes() > 20,
        "the first search left no remainder, so this test checks nothing; change its budget"
    );
    // As `usinewgame` does.
    lean_new_game();
    used.clear();
    let r = used.pending_global_nodes();
    let (got, got_nodes, got_last) = search(&mut used, P, 30_000);

    assert_eq!(got_nodes, want_nodes, "node count depends on the previous search");
    assert_eq!(
        got_last, want_last,
        "the budget stopped the search at another node (previous search's remainder {r})"
    );
    assert_eq!(
        got.as_ref().map(|(m, s)| (m.to_usi_string(), *s)),
        want.as_ref().map(|(m, s)| (m.to_usi_string(), *s)),
        "result depends on the previous search"
    );
}

