"""Mix packed data directories into one, optionally dropping lopsided positions.

    python mix.py --inputs packed_a packed_b --out packed_ab [--max-eval 3000]

Each input is a `nnue_pack` output directory (train.bin, val.bin), already shuffled. The output's
train.bin interleaves the inputs at random in proportion to their (filtered) sizes, which keeps
it shuffled, so the trainer's sequential reads see every source throughout an epoch. val.bin is
mixed the same way. `--max-eval` drops records whose teacher eval exceeds it in absolute value.
`--limits` caps each input's train.bin at that many records (0 = all; inputs are shuffled, so the
first records are a random sample). `lineage.json` in the output records the inputs, their
counts and each input's own `lineage.json`, so a mix carries its whole ancestry
(`docs/nnue-phase2/DATA_LINEAGE.md`); `--note` says what the mix is for.
"""
import argparse
import datetime
import json
import os
import socket
import sys

import numpy as np

from nnue_data import open_records

CHUNK = 1 << 20


def kept(recs, max_eval):
    """The records kept, as a generator of CHUNK-sized slices."""
    for s in range(0, len(recs), CHUNK):
        c = recs[s:s + CHUNK]
        yield c if max_eval is None else c[np.abs(c["eval"].astype(np.int32)) <= max_eval]


def count(recs, max_eval):
    if max_eval is None:
        return len(recs)
    return sum(len(c) for c in kept(recs, max_eval))


def mix(paths, out, max_eval, rng, limits=None):
    srcs = [open_records(p) for p in paths]
    if limits:
        srcs = [r[:k] if k else r for r, k in zip(srcs, limits)]
    n = np.array([count(r, max_eval) for r in srcs], np.int64)
    gens = [kept(r, max_eval) for r in srcs]
    bufs = [r[:0] for r in srcs]
    left = n.copy()
    with open(out, "wb") as f:
        while left.sum() > 0:
            # How many of the next block come from each source, in proportion to what remains
            # (numpy's exact hypergeometric draw is limited to 1e9 records), and in what order.
            need = np.minimum(rng.multinomial(min(CHUNK, int(left.sum())), left / left.sum()), left)
            pick = rng.permutation(np.repeat(np.arange(len(srcs)), need))
            block = np.empty(len(pick), srcs[0].dtype)
            for i in range(len(srcs)):
                while len(bufs[i]) < need[i]:
                    bufs[i] = np.concatenate([bufs[i], next(gens[i])])
                block[pick == i] = bufs[i][:need[i]]
                bufs[i] = bufs[i][need[i]:]
            block.tofile(f)
            left -= need
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--inputs", nargs="+", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--max-eval", type=int)
    ap.add_argument("--limits", type=int, nargs="+", help="per input, max train records (0 = all)")
    ap.add_argument("--seed", type=int, default=39)
    ap.add_argument("--note", default="", help="what this mix is, for lineage.json")
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    rng = np.random.default_rng(a.seed)
    counts = {}
    for name in ("train.bin", "val.bin"):
        n = mix([os.path.join(d, name) for d in a.inputs], os.path.join(a.out, name), a.max_eval, rng,
                a.limits if name == "train.bin" else None)
        counts[name] = [int(k) for k in n]
        print(f"{name}: " + "  ".join(f"{d} {k:,}" for d, k in zip(a.inputs, n)) + f"  total {n.sum():,}",
              flush=True)
    write_lineage(a, counts)


def write_lineage(a, counts):
    inputs = []
    for i, d in enumerate(a.inputs):
        try:
            with open(os.path.join(d, "lineage.json")) as f:
                parent = json.load(f)
        except (OSError, ValueError):
            parent = None  # packed before lineage files existed: see DATA_LINEAGE.md
        inputs.append({"path": os.path.abspath(d), "train": counts["train.bin"][i],
                       "val": counts["val.bin"][i], "limit": a.limits[i] if a.limits else 0,
                       "lineage": parent})
    rec = {"kind": "mix", "tool": "trainer/mix.py", "argv": sys.argv,
           "date": datetime.datetime.now().astimezone().isoformat(timespec="seconds"),
           "host": socket.gethostname(), "note": a.note,
           "options": {"max_eval": a.max_eval, "seed": a.seed},
           "train": sum(counts["train.bin"]), "val": sum(counts["val.bin"]), "inputs": inputs}
    with open(os.path.join(a.out, "lineage.json"), "w") as f:
        json.dump(rec, f, indent=1)
        f.write("\n")


if __name__ == "__main__":
    main()
