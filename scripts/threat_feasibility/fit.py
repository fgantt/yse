import json, math, sys
sys.path.insert(0, sys.argv[1])
from features import features, NAMES
P = lambda cp: 1 / (1 + math.exp(-cp / 410))
rows = [json.loads(l) for l in open(sys.argv[1] + "/scored.jsonl")]
data = []
for r in rows:
    x = features(r["sfen"])
    if x is None: continue
    stm_b = r["sfen"].split()[1] == "b"
    ours_b = r["ours"] if stm_b else -r["ours"]
    r["p_ours"], r["p_yo"] = P(ours_b), P(max(-3000, min(3000, r["yo_black"])))
    r["x"] = x
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
                f = M[i][c] / M[c][c]
                M[i] = [a - f * b_ for a, b_ in zip(M[i], M[c])]
    return [M[i][n] / M[i][i] for i in range(n)]
def fit(cols, lam):
    X = [[z(r["x"])[j] for j in cols] for r in train]; y = [r["p_yo"] - r["p_ours"] for r in train]
    k = len(cols)
    A = [[sum(X[n][i] * X[n][j] for n in range(len(X))) + (lam if i == j and i > 0 else 0) for j in range(k)] for i in range(k)]
    b = [sum(X[n][i] * y[n] for n in range(len(X))) for i in range(k)]
    return solve(A, b)
def evaluate(w, cols, sel):
    if not sel: return None
    before = sum(abs(r["p_ours"] - r["p_yo"]) for r in sel) / len(sel)
    after = 0; bias = 0
    for r in sel:
        zz = z(r["x"]); pred = sum(w[i] * zz[c] for i, c in enumerate(cols))
        pc = min(1, max(0, r["p_ours"] + pred)); after += abs(pc - r["p_yo"]); bias += pc - r["p_yo"]
    return before, after / len(sel), bias / len(sel)
def tier(k):
    if not k: return "none"
    v = k["enemy_near"] + k["enemy_hand"]; return "low" if v <= 1 else "mid" if v <= 3 else "high"
subsets = {
    "hc held-out games": [r for r in data if r["split"] == "test"],
    "hc held-out, |yo|<1500": [r for r in data if r["split"] == "test" and abs(r["yo_black"]) < 1500],
    "user games (all)": [r for r in data if r["split"] == "eg"],
    "user, |yo|<1500 our mid+high": [r for r in data if r["split"] == "eg" and abs(r["yo_black"]) < 1500 and tier(r.get("our_king")) in ("mid", "high")],
    "user, our low": [r for r in data if r["split"] == "eg" and tier(r.get("our_king")) == "low"],
}
models = {"bias only (intercept+stm)": [0, 1], "threat features": list(range(d))}
print(f"train n={len(train)}, features={d}")
for name, cols in models.items():
    w = fit(cols, lam=50.0)
    print(f"\n== {name}")
    for sname, sel in subsets.items():
        e = evaluate(w, cols, sel)
        if e: print(f"  {sname:<32} n={len(sel):>5}  |dp| {e[0]:.3f} -> {e[1]:.3f}   bias after {e[2]:+.3f}")
    if name == "threat features":
        names = ["intercept", "stm"] + [f"-B.{n}" for n in NAMES] + [f"W.{n}" for n in NAMES] + [f"-B.{n}*stm" for n in NAMES] + [f"W.{n}*stm" for n in NAMES]
        top = sorted(zip(names, w), key=lambda t: -abs(t[1]))[:12]
        print("  largest weights:", ", ".join(f"{n} {v:+.3f}" for n, v in top))
