"""Train the engine's NNUE on GPU (CUDA or Apple MPS) or CPU.

    python train.py --data corpus_s38/packed_d8 --tables tables_k9.json --out runs/x \
        [--init net.json] [--epochs 100 --epoch-size 100000000 --lr 1e-3 --gamma 0.97]

Writes `runs/x/epoch_NNN.json` (engine v2 JSON; `nnue-convert-weights` makes the
`.bin`), `runs/x/last.pt` (resume with `--resume`), `runs/x/log.txt` and, when the data has
one, `runs/x/data_lineage.json`.
Target and loss follow the offline trainer's Session 35 recipe: target =
(1 - w) * sigmoid(clamp(eval, 2000) / 410) + w * outcome, prediction =
sigmoid(cp / 410), mean squared error.
"""
import argparse
import os
import shutil
import time

import numpy as np
import torch

from halfkp_model import HalfKPNet, int_eval, write_nnbin
from nnue_data import Features, open_records
from nnue_model import (CP_SCALE, OUT_PER_CP, Net, engine_eval, from_engine, largest_shift,
                        load_engine_json, save_engine_json, to_engine)


def pick_device(name):
    if name != "auto":
        return torch.device(name)
    if torch.cuda.is_available():
        return torch.device("cuda")
    if torch.backends.mps.is_available():
        return torch.device("mps")
    return torch.device("cpu")


def pearson(x, y):
    x, y = x - x.mean(), y - y.mean()
    return float((x * y).sum() / np.sqrt((x * x).sum() * (y * y).sum()))


def engine_eval_batched(q, s_idx, n_idx, chunk=256):
    return np.concatenate([engine_eval(q, s_idx[i:i + chunk], n_idx[i:i + chunk])
                           for i in range(0, len(s_idx), chunk)])


def export(net, feats, sample, path):
    """Round onto the engine's integers at the hidden-layer scales that best
    reproduce the float model on `sample`, write engine JSON, return a report."""
    s_idx, s_sh, n_idx, n_sh = sample
    with torch.no_grad():
        want = net(s_idx, s_sh, n_idx, n_sh).cpu().numpy()
    s_np, n_np = s_idx.cpu().numpy(), n_idx.cpu().numpy()
    best = None
    o = OUT_PER_CP * CP_SCALE
    for alpha in (64.0, 128.0, 256.0, 512.0, 1024.0, 2048.0):
        for beta in (64.0, 256.0, 1024.0, 4096.0):
            # Each layer's weights as large as i16 allows (half of it for layer 3).
            s2 = largest_shift(net.w2, beta / alpha)
            s3 = largest_shift(net.w3, o / beta, limit=16383)
            sm = largest_shift(net.wm, 1.0) if net.hm else 6
            q = to_engine(net, alpha, beta, s2, s3, sm)
            w1 = np.vstack([q["w1"], np.zeros((1, q["w1"].shape[1]), np.int64)])
            acc = w1[np.minimum(s_np, len(q["w1"]))].sum(1) + q["b1"]
            # The engine's fast 16-bit layer-2 path needs activations <= 32767;
            # keep the sample's largest at half that for unseen positions.
            if acc.max() > 16383:
                continue
            err = np.abs(engine_eval_batched(q, s_np, n_np) - want).mean()
            if best is None or err < best[0]:
                best = (err, alpha, beta, q)
    err, alpha, beta, q = best
    save_engine_json(q, path)
    mid = f" mid {q['mid'][0][2]}" if q["mid"] else ""
    return (f"export alpha {alpha:g} beta {beta:g} shifts {q['s2']},{q['s3']}{mid}: "
            f"|int - float| {err:.2f} cp")


def export_halfkp(net, sample, header_from, path):
    """Write the standard format's nn.bin and report how far its integer evaluation is from the
    float model on `sample`."""
    s_idx, s_sh, n_idx, n_sh = sample
    with torch.no_grad():
        want = net(s_idx, s_sh, n_idx, n_sh).cpu().numpy()
    q = net.to_ints()
    got = int_eval(q, s_idx.cpu().numpy(), n_idx.cpu().numpy())
    write_nnbin(q, header_from, path)
    return f"export nn.bin: |int - float| {np.abs(got - want).mean():.2f} (max {np.abs(got - want).max():.0f})"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--data", required=True, help="directory with train.bin and val.bin; several, "
                    "comma-separated: batches are drawn from each in proportion to its size, and "
                    "validation takes an equal share of each one's val.bin")
    ap.add_argument("--tables", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--init", help="start from engine JSON weights instead of scratch")
    ap.add_argument("--resume", action="store_true")
    ap.add_argument("--device", default="auto")
    ap.add_argument("--batch", type=int, default=16384)
    ap.add_argument("--lr", type=float, default=1e-3)
    ap.add_argument("--gamma", type=float, default=0.97, help="lr factor per epoch")
    ap.add_argument("--epochs", type=int, default=100)
    ap.add_argument("--epoch-size", type=int, default=100_000_000)
    ap.add_argument("--outcome-weight", type=float, default=0.3)
    ap.add_argument("--clamp", type=float, default=2000.0)
    ap.add_argument("--sigmoid-scale", type=float, default=410.0)
    ap.add_argument("--val-n", type=int, default=1_000_000)
    ap.add_argument("--seed", type=int, default=38)
    ap.add_argument("--mid", type=int, default=0, help="width of a middle layer (0 = none)")
    ap.add_argument("--arch", choices=["engine", "halfkp"], default="engine",
                    help="halfkp: the standard format (YaneuraOu halfkp_256x2-32-32 nn.bin); "
                         "use tables from `nnue_pack tables --halfkp` and --header-from")
    ap.add_argument("--header-from", help="halfkp: an existing halfkp_256x2-32-32 nn.bin whose "
                    "format header (version, hashes, architecture) the exports reuse")
    a = ap.parse_args()

    os.makedirs(a.out, exist_ok=True)
    log = open(os.path.join(a.out, "log.txt"), "a")
    # The data's ancestry travels with the run (docs/nnue-phase2/DATA_LINEAGE.md).
    dirs = a.data.split(",")
    for i, d in enumerate(dirs):
        if os.path.exists(os.path.join(d, "lineage.json")):
            name = "data_lineage.json" if len(dirs) == 1 else f"data_lineage_{i}.json"
            shutil.copy(os.path.join(d, "lineage.json"), os.path.join(a.out, name))

    def say(msg):
        print(msg, flush=True)
        log.write(msg + "\n")
        log.flush()

    torch.manual_seed(a.seed)
    dev = pick_device(a.device)
    feats = Features(a.tables, dev)
    trains = [open_records(os.path.join(d, "train.bin")) for d in dirs]
    val = np.concatenate([open_records(os.path.join(d, "val.bin"))[:a.val_n // len(dirs)]
                          for d in dirs])
    sizes = np.array([len(t) for t in trains], dtype=np.float64)
    share = sizes / sizes.sum()
    halfkp = a.arch == "halfkp"
    if halfkp:
        assert a.header_from and not a.init, "--arch halfkp needs --header-from, and no --init"
        net = HalfKPNet(feats.rows, feats.stride)
    else:
        net = Net(feats.rows, feats.stride, hm=a.mid, psqt=feats.material_psqt.cpu())
    if a.init:
        q = load_engine_json(a.init)
        from_engine(q, net)
    net.to(dev)
    opt = torch.optim.Adam(net.parameters(), lr=a.lr)
    sched = torch.optim.lr_scheduler.StepLR(opt, step_size=1, gamma=a.gamma)
    rng = np.random.default_rng(a.seed)
    start_epoch, poss = 1, [int(rng.integers(len(t))) for t in trains]
    ckpt = os.path.join(a.out, "last.pt")
    if a.resume and os.path.exists(ckpt):
        st = torch.load(ckpt, map_location=dev)
        net.load_state_dict(st["net"])
        opt.load_state_dict(st["opt"])
        sched.load_state_dict(st["sched"])
        start_epoch, poss = st["epoch"] + 1, st["pos"]
        if isinstance(poss, int):
            poss = [poss]
    say(f"device {dev}  train {int(sizes.sum()):,} ({', '.join(f'{int(n):,}' for n in sizes)})  val {len(val):,}  batch {a.batch}  "
        f"lr {a.lr} gamma {a.gamma}  init {a.init or 'scratch'}  args {vars(a)}")

    val_feats = [feats.batch(val[i:i + a.batch]) for i in range(0, len(val), a.batch)]
    val_teacher = np.clip(val["eval"].astype(np.float64), -a.clamp, a.clamp)
    sample = tuple(t[:4096] for t in val_feats[0][:4])

    def target_of(ev, outcome):
        t = torch.sigmoid(ev.clamp(-a.clamp, a.clamp) / a.sigmoid_scale)
        return (1 - a.outcome_weight) * t + a.outcome_weight * (outcome + 1) / 2

    def validate():
        net.eval()
        with torch.no_grad():
            cp = np.concatenate([net(*b[:4]).cpu().numpy() for b in val_feats])
            loss = np.mean([((torch.sigmoid(net(*b[:4]) / a.sigmoid_scale)
                              - target_of(b[4], b[5])) ** 2).mean().item() for b in val_feats])
        net.train()
        return pearson(cp, val_teacher), loss

    r, vl = validate()
    say(f"epoch {start_epoch - 1:3d}  val pearson[clamp] {r:+.4f}  val loss {vl:.5f}")
    steps = a.epoch_size // a.batch
    for epoch in range(start_epoch, a.epochs + 1):
        t0, total = time.time(), 0.0
        for _ in range(steps):
            k = int(rng.choice(len(trains), p=share)) if len(trains) > 1 else 0
            if poss[k] + a.batch > len(trains[k]):
                poss[k] = 0
            b = feats.batch(trains[k][poss[k]:poss[k] + a.batch])
            poss[k] += a.batch
            pred = torch.sigmoid(net(*b[:4]) / a.sigmoid_scale)
            loss = ((pred - target_of(b[4], b[5])) ** 2).mean()
            opt.zero_grad(set_to_none=True)
            loss.backward()
            opt.step()
            if halfkp:
                net.clamp_()
            total += loss.item()
        sched.step()
        r, vl = validate()
        rate = steps * a.batch / (time.time() - t0)
        if halfkp:
            rep = export_halfkp(net, sample, a.header_from, os.path.join(a.out, f"epoch_{epoch:03d}.bin"))
        else:
            rep = export(net, feats, sample, os.path.join(a.out, f"epoch_{epoch:03d}.json"))
        say(f"epoch {epoch:3d}  train loss {total / steps:.5f}  val pearson[clamp] {r:+.4f}  "
            f"val loss {vl:.5f}  lr {opt.param_groups[0]['lr']:.2e}  {rate:,.0f} pos/s  {rep}")
        torch.save({"net": net.state_dict(), "opt": opt.state_dict(), "sched": sched.state_dict(),
                    "epoch": epoch, "pos": poss}, ckpt)


if __name__ == "__main__":
    main()
