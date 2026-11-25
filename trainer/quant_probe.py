"""Where does a halfkp network lose accuracy on export? Rounds one part of the network at a time onto the
standard format's integer grid and reports, against the float model and against the teacher labels.

    python quant_probe.py --ckpt runs/h1/last.pt --tables tables_halfkp.json --val DIR[,DIR] [--n 20000]

Parts: ft (input layer weights and bias, /127), w1/w2 (hidden weights, /64), t1/t2 (the >>6 truncation of each
hidden layer), out (output weights, 127/16), and all of them (= the exported nn.bin, checked against int_eval).
"""
import argparse
import os

import numpy as np
import torch

from halfkp_model import HalfKPNet, int_eval
from nnue_data import Features, open_records


def pearson(x, y):
    x, y = x - x.mean(), y - y.mean()
    return float((x * y).sum() / np.sqrt((x * x).sum() * (y * y).sum()))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--ckpt", required=True)
    ap.add_argument("--tables", required=True)
    ap.add_argument("--val", required=True)
    ap.add_argument("--n", type=int, default=20000)
    ap.add_argument("--clamp", type=float, default=3000.0)
    a = ap.parse_args()

    feats = Features(a.tables, torch.device("cpu"))
    dirs = a.val.split(",")
    val = np.concatenate([open_records(os.path.join(d, "val.bin"))[:a.n // len(dirs)] for d in dirs])
    s_idx, s_sh, n_idx, n_sh = feats.batch(val)[:4]
    teacher = np.clip(val["eval"].astype(np.float64), -a.clamp, a.clamp)

    net = HalfKPNet(feats.rows, feats.stride)
    net.load_state_dict(torch.load(a.ckpt, map_location="cpu")["net"])
    net.eval()
    with torch.no_grad():
        r = torch.arange(net.rows)
        ftw = (net.w1[:net.rows] + net.shared[r % net.stride]).double().numpy()
        ftb = net.b1.double().numpy()
        W1, B1 = net.l1.weight.double().numpy(), net.l1.bias.double().numpy()
        W2, B2 = net.l2.weight.double().numpy(), net.l2.bias.double().numpy()
        W3, B3 = net.out.weight.double().numpy()[0], float(net.out.bias)
        want = net(s_idx, s_sh, n_idx, n_sh).double().numpy()
    s_np, n_np = s_idx.numpy(), n_idx.numpy()

    def ints(x, scale, lo, hi):
        return np.clip(np.rint(x * scale), lo, hi) / scale

    def forward(q):
        fw = ints(ftw, 127, -32768, 32767) if "ft" in q else ftw
        fb = ints(ftb, 127, -32768, 32767) if "ft" in q else ftb
        fw = np.vstack([fw, np.zeros((1, fw.shape[1]))])

        def acc(idx):
            idx = np.minimum(idx, net.rows)
            return np.clip(np.concatenate([fw[idx[i:i + 500]].sum(1) for i in range(0, len(idx), 500)]) + fb, 0, 1)

        x = np.concatenate([acc(s_np), acc(n_np)], 1)
        w1 = ints(W1, 64, -128, 127) if "w1" in q else W1
        b1 = np.rint(B1 * 64 * 127) / (64 * 127) if "w1" in q else B1
        h = x @ w1.T + b1
        h = np.clip(np.floor(h * 127) / 127 if "t1" in q else h, 0, 1)
        w2 = ints(W2, 64, -128, 127) if "w2" in q else W2
        b2 = np.rint(B2 * 64 * 127) / (64 * 127) if "w2" in q else B2
        h = h @ w2.T + b2
        h = np.clip(np.floor(h * 127) / 127 if "t2" in q else h, 0, 1)
        w3 = np.clip(np.rint(W3 * 16 / 127), -128, 127) * 127 / 16 if "out" in q else W3
        b3 = np.rint(B3 * 16) / 16 if "out" in q else B3
        out = h @ w3 + b3
        return np.trunc(out) if "out" in q else out

    flt = forward(set())
    print(f"n {len(val):,}  float check |numpy - torch| {np.abs(flt - want).mean():.3f}")
    print(f"teacher: pearson float {pearson(flt, teacher):.4f}  |eval| median float {np.median(np.abs(flt)):.0f}"
          f"  teacher {np.median(np.abs(teacher)):.0f}")
    for q in (["ft"], ["w1"], ["t1"], ["w2"], ["t2"], ["out"], ["ft", "w1", "t1", "w2", "t2", "out"]):
        e = forward(set(q))
        d = e - flt
        print(f"{'+'.join(q):22s} |d| mean {np.abs(d).mean():6.2f}  p90 {np.percentile(np.abs(d), 90):6.1f}  "
              f"bias {d.mean():+6.2f}  pearson teacher {pearson(e, teacher):.4f}")
    qi = net.to_ints()
    got = np.concatenate([int_eval(qi, s_np[i:i + 1000], n_np[i:i + 1000]) for i in range(0, len(s_np), 1000)])
    print(f"int_eval vs all-rounded |d| {np.abs(got - forward({'ft', 'w1', 't1', 'w2', 't2', 'out'})).mean():.3f}")
    for name, w, s in (("w1", W1, 64), ("w2", W2, 64), ("out", W3, 16 / 127)):
        i = np.abs(np.rint(w * s))
        print(f"{name}: |int| median {np.median(i):.0f}  p99 {np.percentile(i, 99):.0f}  zero {np.mean(i == 0):.1%}"
              f"  at the i8 limit {np.mean(i >= 127):.1%}")
    i = np.abs(np.rint(ftw * 127))
    print(f"ft: |int| median {np.median(i):.0f}  p99 {np.percentile(i, 99):.0f}  zero {np.mean(i == 0):.1%}")


if __name__ == "__main__":
    main()
