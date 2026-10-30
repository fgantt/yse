"""The engine's network in float, plus import/export of engine weight files.

Engine arithmetic (`src/evaluation/nnue.rs`, `forward_from_activations`):
    a1  = max(0, b1 + sum W1[f])                    per perspective, i16 weights, i32 sums
    h2  = (concat(a1_stm, a1_nstm) @ W2) >> s2 + b2;  a2 = max(0, h2)
    hm  = (a2 @ Wm) >> sm + bm;  a2 = max(0, hm)    only with a middle layer (v6)
    out = b3 + (a2 @ W3) >> s3                       (s2 = s3 = 6 before v5 files)
    cp  = out * 400 / 4080 (truncating) + (psqt_stm - psqt_nstm) / 2 (truncating)

The float model works in natural units (activations of order 1). Export maps it
onto the integers with per-layer scales; because ReLU layers are positively
homogeneous the scale of each hidden layer is free, and `export` picks the one
that rounds best. Rounding happens once, at export, never during training.
"""
import json

import numpy as np
import torch
import torch.nn.functional as F

CP_SCALE = 600.0            # float output unit, in centipawns
OUT_PER_CP = 4080.0 / 400.0  # engine output units per centipawn


class Net(torch.nn.Module):
    def __init__(self, rows, stride, h1=256, h2=32, hm=0, psqt=None):
        """`hm` > 0 adds a middle layer of that width between layer 2 and the output."""
        super().__init__()
        self.hm = hm
        self.rows, self.stride = rows, stride
        self.w1 = torch.nn.Parameter(torch.randn(rows + 1, h1) * 0.01)
        self.shared = torch.nn.Parameter(torch.zeros(stride + 1, h1))   # king factoriser
        self.b1 = torch.nn.Parameter(torch.zeros(h1))
        self.w2 = torch.nn.Parameter(torch.randn(2 * h1, h2) / np.sqrt(2 * h1))
        self.b2 = torch.nn.Parameter(torch.zeros(h2))
        if hm:
            self.wm = torch.nn.Parameter(torch.randn(h2, hm) / np.sqrt(h2))
            self.bm = torch.nn.Parameter(torch.zeros(hm))
        h_last = hm or h2
        self.w3 = torch.nn.Parameter(torch.randn(h_last) / np.sqrt(h_last))
        self.b3 = torch.nn.Parameter(torch.zeros(()))
        self.register_buffer("psqt", torch.zeros(rows + 1) if psqt is None
                             else torch.cat([psqt.float(), torch.zeros(1)]))
        with torch.no_grad():
            self.w1[rows].zero_()
            self.shared[stride].zero_()

    def first_layer(self, idx, sidx):
        return (F.embedding_bag(idx, self.w1, mode="sum", padding_idx=self.rows)
                + F.embedding_bag(sidx, self.shared, mode="sum", padding_idx=self.stride)
                + self.b1)

    def forward(self, s_idx, s_sh, n_idx, n_sh):
        """Centipawns from the side to move's view."""
        a1 = torch.cat([self.first_layer(s_idx, s_sh), self.first_layer(n_idx, n_sh)], 1).relu()
        a2 = (a1 @ self.w2 + self.b2).relu()
        if self.hm:
            a2 = (a2 @ self.wm + self.bm).relu()
        out = a2 @ self.w3 + self.b3
        psqt = (self.psqt[s_idx].sum(1) - self.psqt[n_idx].sum(1)) / 2
        return out * CP_SCALE + psqt

    def coalesced_w1(self):
        """Input rows with the factoriser folded in (what the engine sees)."""
        r = torch.arange(self.rows, device=self.w1.device)
        return self.w1[:self.rows] + self.shared[r % self.stride]


# --- engine weight files ---------------------------------------------------------

FEATURE_SET = {"pov": True, "stm": False, "halfkp": False, "hand": True, "king": True, "dual": True}


def load_engine_json(path):
    """An engine v2 JSON file (`nnue-convert-weights --output x.json`) as int64 arrays."""
    j = json.load(open(path))
    w = j["weights"]
    return {
        "w1": np.array(w["input_weights_1"], dtype=np.int64),
        "b1": np.array(w["hidden_biases_1"], dtype=np.int64),
        "w2": np.array(w["input_weights_2"], dtype=np.int64),
        "b2": np.array(w["hidden_biases_2"], dtype=np.int64),
        "w3": np.array(w["output_weights"], dtype=np.int64),
        "b3": int(w["output_bias"]),
        "psqt": np.array(j.get("psqt", []), dtype=np.int64),
        "feature_set": j["feature_set"],
        "s2": (j.get("shifts") or [6, 6])[0],
        "s3": (j.get("shifts") or [6, 6])[1],
        "mid": [(np.array(m["weights"], dtype=np.int64), np.array(m["biases"], dtype=np.int64),
                 int(m["shift"])) for m in j.get("mid_layers", [])],
    }


def save_engine_json(q, path):
    out = {
        "feature_set": FEATURE_SET,
        "weights": {
            "input_weights_1": q["w1"].tolist(),
            "hidden_biases_1": q["b1"].tolist(),
            "input_weights_2": q["w2"].tolist(),
            "hidden_biases_2": q["b2"].tolist(),
            "output_weights": q["w3"].tolist(),
            "output_bias": int(q["b3"]),
        },
        "psqt": q["psqt"].tolist(),
    }
    if (q["s2"], q["s3"]) != (6, 6) or q.get("mid"):
        out["shifts"] = [q["s2"], q["s3"]]
    if q.get("mid"):
        out["mid_layers"] = [{"weights": w.tolist(), "biases": b.tolist(), "shift": s}
                             for w, b, s in q["mid"]]
    json.dump(out, open(path, "w"))


def _trunc_div(a, b):
    """Integer division truncating toward zero, as Rust's `/`."""
    q = np.abs(a) // abs(b)
    return np.where((a < 0) != (b < 0), -q, q)


def engine_eval(q, s_idx, n_idx):
    """The engine's integer evaluation of a batch (numpy int64; padded indices
    must equal `len(q['w1'])` and are dropped)."""
    rows = len(q["w1"])
    w1 = np.vstack([q["w1"], np.zeros((1, q["w1"].shape[1]), np.int64)])
    psqt = np.concatenate([q["psqt"], [0]]) if len(q["psqt"]) else np.zeros(rows + 1, np.int64)
    s_idx, n_idx = np.minimum(s_idx, rows), np.minimum(n_idx, rows)
    a1 = np.concatenate([np.maximum(w1[s_idx].sum(1) + q["b1"], 0),
                         np.maximum(w1[n_idx].sum(1) + q["b1"], 0)], 1)
    a2 = np.maximum(((a1 @ q["w2"]) >> q["s2"]) + q["b2"], 0)
    for w, b, s in q.get("mid", []):
        a2 = np.maximum(((a2 @ w) >> s) + b, 0)
    out = q["b3"] + ((a2 @ q["w3"]) >> q["s3"])
    return _trunc_div(out * 400, 4080) + _trunc_div(psqt[s_idx].sum(1) - psqt[n_idx].sum(1), 2)


def from_engine(q, net, alpha=64.0, beta=64.0):
    """Load integer weights into the float model. `alpha`, `beta`: integer units
    per float unit of the first and second hidden layers (and of the middle one)."""
    assert len(q.get("mid", [])) == (1 if net.hm else 0), "middle layers do not match the model"
    o = OUT_PER_CP * CP_SCALE                 # engine output units per float output unit
    t = lambda a: torch.tensor(np.asarray(a, dtype=np.float64), dtype=torch.float32)
    with torch.no_grad():
        net.w1[:net.rows] = t(q["w1"]) / alpha
        net.w1[net.rows].zero_()
        net.shared.zero_()
        net.b1.copy_(t(q["b1"]) / alpha)
        net.w2.copy_(t(q["w2"]) * alpha / (2 ** q["s2"] * beta))
        net.b2.copy_(t(q["b2"]) / beta)
        for w, b, s in q.get("mid", []):
            net.wm.copy_(t(w) / 2 ** s)
            net.bm.copy_(t(b) / beta)
        net.w3.copy_(t(q["w3"]) * beta / (2 ** q["s3"] * o))
        net.b3.copy_(torch.tensor(q["b3"] / o))
        if len(q["psqt"]):
            net.psqt[:net.rows] = t(q["psqt"])


def to_engine(net, alpha, beta, s2=6, s3=6, sm=6):
    """Round the float model onto the integers at layer scales `alpha`, `beta`
    (`beta` for the middle layer too) and layer shifts `s2`, `s3`, `sm`."""
    o = OUT_PER_CP * CP_SCALE
    c = lambda x, lo, hi: np.clip(np.rint(x.detach().cpu().double().numpy()), lo, hi).astype(np.int64)
    i16, i32 = (-32767, 32767), (-(2 ** 31) + 1, 2 ** 31 - 1)
    return {
        "w1": c(net.coalesced_w1() * alpha, *i16),
        "b1": c(net.b1 * alpha, *i32),
        "w2": c(net.w2 * 2 ** s2 * beta / alpha, *i16),
        "b2": c(net.b2 * beta, *i32),
        "w3": c(net.w3 * 2 ** s3 * o / beta, *i16),
        "b3": int(np.rint(net.b3.item() * o)),
        "psqt": np.rint(net.psqt[:net.rows].cpu().numpy()).astype(np.int64),
        "s2": s2,
        "s3": s3,
        "mid": [(c(net.wm * 2 ** sm, *i16), c(net.bm * beta, *i32), sm)] if net.hm else [],
    }


def largest_shift(w, scale, limit=32767, lo=6, hi=24):
    """The largest shift s in [lo, hi] with max |w * 2**s * scale| <= limit."""
    m = float(w.detach().abs().max()) * scale
    s = lo
    while s < hi and m * 2 ** (s + 1) <= limit:
        s += 1
    return s
