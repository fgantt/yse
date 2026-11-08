//! The lean search's tunable constants (`SHOGI_LEAN_PARAMS`), for SPSA tuning.
//!
//! Session 40 (`RESEARCH_S40.md` §4 C): our pruning margins were set by hand,
//! many inherited from chess practice; YaneuraOu's 2025 search work found
//! chess-derived constants wrong for shogi and gained most by retuning them.
//! The defaults are SPSA run v1's values (`matches/s40/spsa_v1`, 20,000 pairs),
//! shipped with correction history and the helper-table fix after the stack
//! passed the 1 s gate vs stable (Session 40). The hand-set values before it were
//! rfp_margin 120, razor_margin 250, null_eval_div 200, probcut_margin 200,
//! hist_prune_mul 4000, fut_base 100, fut_mul 150, see_quiet_mul 80,
//! see_cap_mul 150, lmr_div 225, lmr_hist_div 8000.
//!
//! `SHOGI_LEAN_PARAMS="rfp_margin=110,fut_mul=140"`: comma-separated
//! `name=integer`; unknown names are an error (printed once, then ignored), so a
//! tuning run cannot silently tune nothing. Read once per process.

macro_rules! params {
    ($($name:ident = $default:expr, $doc:literal;)*) => {
        #[derive(Clone, Debug)]
        pub struct LeanParams { $(#[doc = $doc] pub $name: i32,)* }

        impl Default for LeanParams {
            fn default() -> Self { Self { $($name: $default,)* } }
        }

        impl LeanParams {
            fn set(&mut self, name: &str, v: i32) -> bool {
                match name { $(stringify!($name) => { self.$name = v; true })* _ => false }
            }

            /// Every parameter as (name, value), in declaration order.
            pub fn list(&self) -> Vec<(&'static str, i32)> {
                vec![$((stringify!($name), self.$name),)*]
            }
        }
    };
}

params! {
    rfp_depth = 6, "Reverse futility: deepest remaining depth.";
    rfp_margin = 127, "Reverse futility: margin per ply.";
    razor_depth = 2, "Razoring: deepest remaining depth.";
    razor_margin = 229, "Razoring: margin per ply.";
    null_depth = 3, "Null move: shallowest remaining depth.";
    null_base = 3, "Null move: base reduction.";
    null_depth_div = 3, "Null move: one more ply of reduction per this many plies.";
    null_eval_div = 194, "Null move: one more ply per this many cp above beta...";
    null_eval_max = 3, "...up to this many.";
    probcut_depth = 5, "ProbCut: shallowest remaining depth.";
    probcut_margin = 206, "ProbCut: margin over beta.";
    hist_prune_depth = 3, "History pruning of quiet moves: deepest remaining depth.";
    hist_prune_mul = 3584, "History pruning: threshold per ply (negated).";
    lmp_base = 3, "Late-move pruning: moves allowed = base + depth^2.";
    lmp_depth = 8, "Late-move pruning: deepest remaining depth.";
    fut_depth = 6, "Futility: deepest remaining depth.";
    fut_base = 106, "Futility: base margin.";
    fut_mul = 153, "Futility: margin per ply.";
    see_quiet_depth = 6, "SEE pruning of quiet moves and drops: deepest depth.";
    see_quiet_mul = 86, "SEE pruning of quiet moves: loss allowed per ply.";
    see_cap_depth = 6, "SEE pruning of captures: deepest depth.";
    see_cap_mul = 171, "SEE pruning of captures: loss allowed per ply.";
    lmr_div = 226, "LMR table: ln(d) ln(m) / (lmr_div / 100).";
    lmr_hist_div = 8123, "LMR: one ply less per this much quiet history.";
}

/// The process's parameters.
pub fn lean_params() -> &'static LeanParams {
    static P: std::sync::OnceLock<LeanParams> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let mut p = LeanParams::default();
        if let Ok(spec) = std::env::var("SHOGI_LEAN_PARAMS") {
            for item in spec.split(',').map(str::trim).filter(|s| !s.is_empty()) {
                let ok = item
                    .split_once('=')
                    .and_then(|(k, v)| v.trim().parse().ok().map(|v| (k.trim(), v)))
                    .is_some_and(|(k, v)| p.set(k, v));
                if !ok {
                    eprintln!("SHOGI_LEAN_PARAMS: ignored '{item}' (unknown name or bad value)");
                }
            }
        }
        // Divisors must stay positive.
        p.null_depth_div = p.null_depth_div.max(1);
        p.null_eval_div = p.null_eval_div.max(1);
        p.lmr_div = p.lmr_div.max(1);
        p.lmr_hist_div = p.lmr_hist_div.max(1);
        p
    })
}
