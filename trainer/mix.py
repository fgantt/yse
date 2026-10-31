"""Mix packed data directories into one, optionally dropping lopsided positions.

    python mix.py --inputs packed_a packed_b --out packed_ab [--max-eval 3000]

Each input is a `nnue_pack` output directory (train.bin, val.bin), already shuffled. The output's
train.bin interleaves the inputs at random in proportion to their (filtered) sizes, which keeps
it shuffled, so the trainer's sequential reads see every source throughout an epoch. val.bin is
mixed the same way. `--max-eval` drops records whose teacher eval exceeds it in absolute value.
"""
import argparse
import os

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


def mix(paths, out, max_eval, rng):
    srcs = [open_records(p) for p in paths]
    n = np.array([count(r, max_eval) for r in srcs], np.int64)
    gens = [kept(r, max_eval) for r in srcs]
    bufs = [r[:0] for r in srcs]
    left = n.copy()
    with open(out, "wb") as f:
        while left.sum() > 0:
            # How many of the next block come from each source (exact, never more than remains),
            # and in what order.
            need = rng.multivariate_hypergeometric(left, min(CHUNK, int(left.sum())))
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
    ap.add_argument("--seed", type=int, default=39)
    a = ap.parse_args()
    os.makedirs(a.out, exist_ok=True)
    rng = np.random.default_rng(a.seed)
    for name in ("train.bin", "val.bin"):
        n = mix([os.path.join(d, name) for d in a.inputs], os.path.join(a.out, name), a.max_eval, rng)
        print(f"{name}: " + "  ".join(f"{d} {k:,}" for d, k in zip(a.inputs, n)) + f"  total {n.sum():,}",
              flush=True)


if __name__ == "__main__":
    main()
