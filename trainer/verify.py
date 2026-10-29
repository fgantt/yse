"""Check the trainer against the engine on `nnue_pack check` output:
features identical, integer emulation equal to the engine's evaluation, and the
float model close to it.

    python verify.py --check check.jsonl --tables tables_k9.json --weights net.json
"""
import argparse
import json

import numpy as np
import torch

from nnue_data import DTYPE, Features
from nnue_model import Net, engine_eval, from_engine, load_engine_json


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", required=True)
    ap.add_argument("--tables", required=True)
    ap.add_argument("--weights", help="engine JSON weights the check file was evaluated with")
    a = ap.parse_args()
    rows = [json.loads(l) for l in open(a.check)]
    rec = np.frombuffer(b"".join(bytes.fromhex(r["rec"]) for r in rows), dtype=DTYPE)
    feats = Features(a.tables, "cpu")
    s_idx, s_sh, n_idx, n_sh, ev, _ = feats.batch(rec)
    bad = 0
    for i, r in enumerate(rows):
        for got, want in ((s_idx[i], r["stm"]), (n_idx[i], r["nstm"])):
            g = sorted(x for x in got.tolist() if x != feats.pad)
            if g != sorted(want):
                bad += 1
                if bad <= 3:
                    print("feature mismatch at", i, sorted(set(g) ^ set(want))[:10])
    print(f"features: {len(rows)} positions, {bad} mismatching perspectives")
    sh_ok = bool(((s_sh == feats.shared_pad) == (s_idx == feats.pad)).all()
                 and (torch.where(s_idx < feats.pad, s_idx % feats.stride, s_sh) == s_sh).all())
    print("factoriser rows consistent:", sh_ok)
    if not a.weights:
        return
    q = load_engine_json(a.weights)
    want = np.array([r["eval"] for r in rows])
    got = engine_eval(q, s_idx.numpy(), n_idx.numpy())
    diff = got - want
    print(f"integer emulation vs engine: {np.count_nonzero(diff)} of {len(rows)} differ, "
          f"max |diff| {np.abs(diff).max()}")
    net = Net(feats.rows, feats.stride, psqt=torch.tensor(q["psqt"]))
    from_engine(q, net)
    with torch.no_grad():
        f = net(s_idx, s_sh, n_idx, n_sh).numpy()
    print(f"float model vs engine: mean |diff| {np.abs(f - want).mean():.2f} cp, "
          f"max {np.abs(f - want).max():.1f} cp")


if __name__ == "__main__":
    main()
