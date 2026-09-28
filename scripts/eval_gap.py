"""Measure an engine against the eval-divergence set (see eval_gap_build.py).

Each position is searched at a fixed depth, single-threaded, from a fresh
game (so results do not depend on order), and compared with YaneuraOu's label
in win-probability space, p = 1 / (1 + exp(-cp / 410)) -- the trainer's
scale. In won positions our centipawns run at about half YaneuraOu's, which a
raw cp error would mostly measure; in p both are near 1 and the error there is
small, so what remains is the misjudgement that changes results.

Subsets come from board-only tags fixed when the set was built, never from
our engine's scores:
  pressure on a king = enemy pieces within two squares + enemy non-pawn
  pieces in hand;  low 0-1, mid 2-3, high >= 4.

    python3 scripts/eval_gap.py ENGINE [--depth 1] [--jobs 4] [--csv out.csv] [--option NNUEFile=x.bin]
    python3 scripts/eval_gap.py --compare a.csv b.csv   # paired, per subset
"""
import argparse, csv, json, math, os, subprocess, sys
from concurrent.futures import ProcessPoolExecutor

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCALE, CLAMP = 410.0, 3000


def p(cp):
    return 1.0 / (1.0 + math.exp(-cp / SCALE))


def tier(k):
    if not k:
        return "none"
    v = k["enemy_near"] + k["enemy_hand"]
    return "low" if v <= 1 else "mid" if v <= 3 else "high"


def run_chunk(args):
    engine, depth, options, rows = args
    e = subprocess.Popen([engine], cwd=ROOT, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, text=True, bufsize=1)
    send = lambda s: (e.stdin.write(s + "\n"), e.stdin.flush())

    def until(tok):
        out = []
        while True:
            line = e.stdout.readline()
            if not line:
                raise RuntimeError("engine exited")
            out.append(line.strip())
            if line.startswith(tok):
                return out

    send("usi"); until("usiok")
    send("setoption name ParallelEnable value false")
    for o in options:
        name, value = o.split("=", 1)
        send(f"setoption name {name} value {value}")
    send("isready"); until("readyok")
    res = []
    for r in rows:
        send("usinewgame"); send(f"position sfen {r['sfen']}"); send(f"go depth {depth}")
        out = until("bestmove")
        cp = None
        for l in out:
            t = l.split()
            if t[:1] == ["info"] and "score" in t:
                i = t.index("score")
                v = int(t[i + 2])
                cp = v if t[i + 1] == "cp" else (CLAMP if v > 0 else -CLAMP)
        best = out[-1].split()[1]
        if cp is None:  # no legal move: mated
            cp = -CLAMP
        cp = max(-CLAMP, min(CLAMP, cp))
        ours = cp if r["stm"] == "b" else -cp
        res.append({"game": r["game"], "ply": r["ply"], "stm": r["stm"],
                    "yo": r["yo_cp"], "ours": ours, "best": best, "yo_best": r["yo_best"],
                    "our_tier": tier(r["our_king"]), "their_tier": tier(r["their_king"])})
    send("quit")
    return res


def summarize(rows, label=""):
    def line(name, sel):
        if not sel:
            return
        err = [p(r["ours"]) - p(r["yo"]) for r in sel]
        ours_to_move = [r for r in sel if r["stm"] == "b"]
        agree = sum(r["best"] == r["yo_best"] for r in ours_to_move)
        print(f"  {name:<24} n={len(sel):>4}  mean|dp| {sum(map(abs, err)) / len(err):.3f}  "
              f"mean dp {sum(err) / len(err):+.3f}  "
              f"best-move agree {agree}/{len(ours_to_move)}")
    print(label)
    line("all", rows)
    line("contested |yo|<1500", [r for r in rows if abs(int(r["yo"])) < 1500])
    for t in ["low", "mid", "high"]:
        line(f"our king pressure {t}", [r for r in rows if r["our_tier"] == t])
    for t in ["low", "mid", "high"]:
        line(f"their king pressure {t}", [r for r in rows if r["their_tier"] == t])
    # The blind spot: close positions with our king under attack. In won positions
    # both p are near 1, so pressure alone partly measures centipawn scale.
    line("|yo|<1500, our mid+high", [r for r in rows if abs(int(r["yo"])) < 1500
                                      and r["our_tier"] in ("mid", "high")])
    line("|yo|<1500, our low", [r for r in rows if abs(int(r["yo"])) < 1500
                                 and r["our_tier"] == "low"])
    print("  (dp = p(ours) - p(YaneuraOu); positive = we are more optimistic for Black)")


def compare(a_path, b_path):
    load = lambda f: {(r["game"], r["ply"]): r for r in csv.DictReader(open(f))}
    a, b = load(a_path), load(b_path)
    keys = sorted(a.keys() & b.keys())
    subsets = {"all": keys,
               "contested |yo|<1500": [k for k in keys if abs(int(a[k]["yo"])) < 1500]}
    for t in ["mid", "high"]:
        subsets[f"our king pressure {t}"] = [k for k in keys if a[k]["our_tier"] == t]
    subsets["|yo|<1500, our mid+high"] = [k for k in subsets["contested |yo|<1500"]
                                          if a[k]["our_tier"] in ("mid", "high")]
    print(f"paired |dp| change, {b_path} minus {a_path} (negative = B closer to YaneuraOu)")
    for name, ks in subsets.items():
        d = [abs(p(int(b[k]["ours"])) - p(int(b[k]["yo"])))
             - abs(p(int(a[k]["ours"])) - p(int(a[k]["yo"]))) for k in ks]
        if len(d) < 2:
            continue
        m = sum(d) / len(d)
        se = math.sqrt(sum((x - m) ** 2 for x in d) / (len(d) - 1) / len(d))
        print(f"  {name:<24} n={len(d):>4}  {m:+.4f} +/- {se:.4f} (SE)")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("engine", nargs="?")
    ap.add_argument("--set", default=os.path.join(ROOT, "eval_gap/positions.jsonl"))
    ap.add_argument("--depth", type=int, default=1)
    ap.add_argument("--jobs", type=int, default=4)
    ap.add_argument("--csv")
    ap.add_argument("--option", action="append", default=[],
                    help="USI option NAME=VALUE, e.g. NNUEFile=weights/x.bin (repeatable)")
    ap.add_argument("--compare", nargs=2)
    a = ap.parse_args()
    if a.compare:
        return compare(*a.compare)
    if not a.engine:
        sys.exit("engine required")
    rows = [json.loads(l) for l in open(a.set)]
    chunks = [(os.path.abspath(a.engine), a.depth, a.option, rows[i::a.jobs]) for i in range(a.jobs)]
    with ProcessPoolExecutor(a.jobs) as ex:
        res = [r for part in ex.map(run_chunk, chunks) for r in part]
    res.sort(key=lambda r: (r["game"], r["ply"]))
    if a.csv:
        with open(a.csv, "w", newline="") as f:
            w = csv.DictWriter(f, fieldnames=list(res[0].keys()))
            w.writeheader(); w.writerows(res)
    summarize(res, f"{a.engine} {' '.join(a.option)} depth {a.depth}, {len(res)} positions")


if __name__ == "__main__":
    main()
