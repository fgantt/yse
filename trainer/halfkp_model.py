"""The standard format's network (YaneuraOu `halfkp_256x2-32-32`, `eval/nn.bin`) in float, and its export.

Integer arithmetic of the format, as our loader implements it (`src/evaluation/yo_nnue.rs`):
    acc  = ft_bias + sum ft_w[king * 1548 + f]            i16, per perspective (side to move first)
    in   = clamp(acc, 0, 127)                              512 u8
    h1   = clamp((b1 + W1 . in) >> 6, 0, 127)              32, i8 weights, i32 biases
    h2   = clamp((b2 + W2 . h1) >> 6, 0, 127)              32
    eval = (b3 + w3 . h2) / 16                             the score, side to move's view

The float model mirrors it with activations in [0, 1] (one unit = 127 on the integer grid), so the
integer weights are fixed multiples of the float ones:
    ft_w = 127 w, ft_bias = 127 b;  W1, W2 = 64 w (i8, so |w| <= 127/64), b1, b2 = 64 * 127 b;
    w3 = 16 w / 127 (i8, so |w| <= 127 * 127 / 16), b3 = 16 b.
Hidden weights are clamped to their i8 range after every step (`clamp_`), so export only rounds.
The input layer has a king-free factoriser (`shared`, one row per BonaPiece), folded in at export.
These relations follow from the arithmetic above; no other trainer's code or constants are used.
"""
import struct

import numpy as np
import torch
import torch.nn.functional as F

H1, H2, H3 = 256, 32, 32
W_HIDDEN = 127.0 / 64.0
W_OUT = 127.0 * 127.0 / 16.0


class HalfKPNet(torch.nn.Module):
    def __init__(self, rows, stride):
        super().__init__()
        self.rows, self.stride = rows, stride
        self.w1 = torch.nn.Parameter(torch.randn(rows + 1, H1) * 0.01)
        self.shared = torch.nn.Parameter(torch.zeros(stride + 1, H1))
        self.b1 = torch.nn.Parameter(torch.full((H1,), 0.5))
        self.l1 = torch.nn.Linear(2 * H1, H2)
        self.l2 = torch.nn.Linear(H2, H3)
        self.out = torch.nn.Linear(H3, 1)
        with torch.no_grad():
            self.w1[rows].zero_()
            self.shared[stride].zero_()
            for l in (self.l1, self.l2):
                l.weight.mul_(0.5)
                l.bias.zero_()
            self.out.weight.normal_(0, 100.0)
            self.out.bias.zero_()

    def first_layer(self, idx, sidx):
        return (F.embedding_bag(idx, self.w1, mode="sum", padding_idx=self.rows)
                + F.embedding_bag(sidx, self.shared, mode="sum", padding_idx=self.stride)
                + self.b1)

    def forward(self, s_idx, s_sh, n_idx, n_sh):
        """Score in the format's units (side to move's view)."""
        a = torch.cat([self.first_layer(s_idx, s_sh), self.first_layer(n_idx, n_sh)], 1).clamp(0, 1)
        a = self.l1(a).clamp(0, 1)
        a = self.l2(a).clamp(0, 1)
        return self.out(a).squeeze(1)

    def clamp_(self):
        with torch.no_grad():
            self.l1.weight.clamp_(-W_HIDDEN, W_HIDDEN)
            self.l2.weight.clamp_(-W_HIDDEN, W_HIDDEN)
            self.out.weight.clamp_(-W_OUT, W_OUT)

    def to_ints(self):
        """Integer weights of the format, as numpy arrays."""
        with torch.no_grad():
            r = torch.arange(self.rows, device=self.w1.device)
            w = (self.w1[:self.rows] + self.shared[r % self.stride]).cpu().numpy()
            q = {
                "ft_bias": np.clip(np.rint(self.b1.cpu().numpy() * 127), -32768, 32767).astype("<i2"),
                "ft_w": np.clip(np.rint(w * 127), -32768, 32767).astype("<i2"),
                "b1": np.rint(self.l1.bias.cpu().numpy() * 64 * 127).astype("<i4"),
                "w1": np.clip(np.rint(self.l1.weight.cpu().numpy() * 64), -128, 127).astype("i1"),
                "b2": np.rint(self.l2.bias.cpu().numpy() * 64 * 127).astype("<i4"),
                "w2": np.clip(np.rint(self.l2.weight.cpu().numpy() * 64), -128, 127).astype("i1"),
                "b3": np.rint(self.out.bias.cpu().numpy() * 16).astype("<i4"),
                "w3": np.clip(np.rint(self.out.weight.cpu().numpy() * 16 / 127), -128, 127).astype("i1"),
            }
        return q


def int_eval(q, s_idx, n_idx):
    """The format's integer evaluation of a batch (indices as the trainer builds them; padding =
    rows). Checks the export against the float model; `src/evaluation/yo_nnue.rs` does the same sums."""
    rows = q["ft_w"].shape[0]
    ft = np.vstack([q["ft_w"].astype(np.int64), np.zeros((1, H1), np.int64)])

    def acc(idx):
        return np.clip(ft[np.minimum(idx, rows)].sum(1) + q["ft_bias"].astype(np.int64), 0, 127)

    x = np.concatenate([acc(s_idx), acc(n_idx)], 1)
    h1 = np.clip((x @ q["w1"].astype(np.int64).T + q["b1"]) >> 6, 0, 127)
    h2 = np.clip((h1 @ q["w2"].astype(np.int64).T + q["b2"]) >> 6, 0, 127)
    out = h2 @ q["w3"].astype(np.int64).T + q["b3"]
    return np.trunc(out[:, 0] / 16.0)


def read_header(path):
    """(prefix, net hash) of a `halfkp_256x2-32-32` file: version, hash, architecture string and
    feature-transformer hash, which YaneuraOu checks on load; our files reuse them so they load."""
    b = open(path, "rb").read()
    arch_len = struct.unpack("<I", b[8:12])[0]
    prefix = b[:12 + arch_len + 4]
    arch = b[12:12 + arch_len].decode("ascii")
    assert arch.startswith("Features=HalfKP(Friend)[125388->256x2]"), arch
    net_hash = b[len(prefix) + 2 * H1 + 2 * H1 * 125388:][:4]
    return prefix, net_hash


def write_nnbin(q, header_from, path):
    prefix, net_hash = read_header(header_from)
    with open(path, "wb") as f:
        f.write(prefix)
        f.write(q["ft_bias"].tobytes())
        f.write(q["ft_w"].tobytes())          # row-major: input row, then 256 outputs
        f.write(net_hash)
        f.write(q["b1"].tobytes())
        f.write(q["w1"].tobytes())            # [32][512], output-major
        f.write(q["b2"].tobytes())
        f.write(q["w2"].tobytes())
        f.write(q["b3"].tobytes())
        f.write(q["w3"].tobytes())
