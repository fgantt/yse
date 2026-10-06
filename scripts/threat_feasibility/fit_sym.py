"""Colour-symmetric threat correction: ridge of p(YaneuraOu) - p(ours), both from
the side to move's view, on features_stm. Usage: fit_sym.py DIR [coef_out.json]
DIR holds scored.jsonl (handicap + eval-gap) and scored_even.jsonl (even games)."""
import json, math, sys, os
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from features import features_stm, NAMES
P = lambda cp: 1 / (1 + math.exp(-cp / 410))
D = sys.argv[1]
rows = [json.loads(l) for f in ("scored.jsonl", "scored_even.jsonl") for l in open(os.path.join(D, f))]
data = []
for r in rows:
    x = features_stm(r["sfen"])
    if x is None:
        continue
    stm_b = r["sfen"].split()[1] == "b"
    yo = max(-3000, min(3000, r["yo_black"]))
    r["p_ours"], r["p_yo"] = P(r["ours"]), P(yo if stm_b else -yo)  # ours is already stm view
    r["x"] = x
    r.setdefault("src", "hc")
    data.append(r)
train = [r for r in data if r["split"] == "train"]
d = len(train[0]["x"])
mu = [sum(r["x"][j] for r in train) / len(train) for j in range(d)]
sd = [max(1e-9, math.sqrt(sum((r["x"][j] - mu[j]) ** 2 for r in train) / len(train))) for j in range(d)]
def z(x): return [1.0] + [(x[j] - mu[j]) / sd[j] for j in range(1, d)]
def solve(A, b):
    n = len(b); M = [A[i][:] + [b[i]] for i in range(n)]
    for c in range(n):
        p = max(range(c, n), key=lambda i: abs(M[i][c])); M[c], M[p] = M[p], M[c]
        for i in range(n):
            if i != c and M[i][c]:
                f = M[i][c] / M[c][c]; M[i] = [a - f * b_ for a, b_ in zip(M[i], M[c])]
    return [M[i][n] / M[i][i] for i in range(n)]
Z = [z(r["x"]) for r in train]; Y = [r["p_yo"] - r["p_ours"] for r in train]
lam = 50.0
A = [[sum(Z[n][i] * Z[n][j] for n in range(len(Z))) + (lam if i == j and i > 0 else 0) for j in range(d)] for i in range(d)]
b = [sum(Z[n][i] * Y[n] for n in range(len(Z))) for i in range(d)]
w = solve(A, b)
coef = [w[0] - sum(w[j] * mu[j] / sd[j] for j in range(1, d))] + [w[j] / sd[j] for j in range(1, d)]
def pred(r): return sum(c * v for c, v in zip(coef, r["x"]))
def tier(k):
    if not k: return "none"
    v = k["enemy_near"] + k["enemy_hand"]; return "low" if v <= 1 else "mid" if v <= 3 else "high"
def report(name, sel):
    if not sel: return
    before = sum(abs(r["p_ours"] - r["p_yo"]) for r in sel) / len(sel)
    after = sum(abs(min(1, max(0, r["p_ours"] + pred(r))) - r["p_yo"]) for r in sel) / len(sel)
    print(f"  {name:<34} n={len(sel):>5}  |dp| {before:.3f} -> {after:.3f}")
print(f"train n={len(train)} (handicap {sum(r['src']=='hc' for r in train)}, even {sum(r['src']=='even' for r in train)}), features={d}")
report("held-out handicap games", [r for r in data if r["src"] == "hc" and r["split"] == "test"])
report("held-out even games", [r for r in data if r["src"] == "even" and r["split"] == "test"])
report("user games, all", [r for r in data if r["split"] == "eg"])
report("user games, our king high", [r for r in data if r["split"] == "eg" and tier(r.get("our_king")) == "high"])
report("user, |yo|<1500 our mid+high", [r for r in data if r["split"] == "eg" and abs(r["yo_black"]) < 1500 and tier(r.get("our_king")) in ("mid", "high")])
for name, sf in [("startpos, Black to move", "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1"),
                 ("startpos, White to move", "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1")]:
    p = sum(c * v for c, v in zip(coef, features_stm(sf)))
    print(f"  {name}: correction {p:+.4f} (side to move's p)")
names = ["intercept"] + [f"us.{n}" for n in NAMES] + [f"them.{n}" for n in NAMES]
print("  largest standardised weights:", ", ".join(f"{n} {v:+.3f}" for n, v in sorted(zip(names, w), key=lambda t: -abs(t[1]))[:10]))
if len(sys.argv) > 2:
    json.dump({"coef": coef, "names": names}, open(sys.argv[2], "w"))
