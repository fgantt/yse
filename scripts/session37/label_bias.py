# How far depth-8 (and 12) labels sit from depth 16, by position type. Scores are from the side to
# move; positive bias = the shallow label flatters the side to move.
# usage: python3 scripts/session37/label_bias.py LABELLED.jsonl
import json, sys, statistics as st
rows = [json.loads(l) for l in open(sys.argv[1])]
rows = [r for r in rows if all(r.get(f"d{d}") is not None and abs(r[f"d{d}"]) < 15000 for d in (8, 12, 16))]
def hand(sfen):
    h = sfen.split()[2]; n = 0; k = ""
    for c in h:
        if c.isdigit(): k += c
        elif c != "-": n += int(k or 1); k = ""
    return n
def line(name, rs):
    if len(rs) < 30: return
    a = [r["d8"] - r["d16"] for r in rs]; b = [r["d12"] - r["d16"] for r in rs]
    big = sum(x >= 300 for x in a) / len(a); low = sum(x <= -300 for x in a) / len(a)
    print(f"  {name:34s} n={len(rs):6d}  d8-d16 mean {st.mean(a):+5.0f} med {st.median(a):+4.0f} "
          f"|err| {st.mean(map(abs, a)):4.0f}  >=+300 {100*big:4.1f}% <=-300 {100*low:4.1f}%  | d12-d16 mean {st.mean(b):+5.0f} |err| {st.mean(map(abs, b)):4.0f}")
print(f"{len(rows)} positions without mate scores")
line("all", rows)
for s in "bw": line(f"side to move {s}", [r for r in rows if r["sfen"].split()[1] == s])
for lo, hi in ((1, 40), (40, 80), (80, 120), (120, 999)): line(f"ply {lo}-{hi - 1}", [r for r in rows if lo <= r.get("ply", 0) < hi])
for lo, hi in ((0, 300), (300, 800), (800, 1500), (1500, 15000)): line(f"|d16| {lo}-{hi}", [r for r in rows if lo <= abs(r["d16"]) < hi])
for lo, hi in ((0, 3), (3, 6), (6, 10), (10, 99)): line(f"pieces in hand {lo}-{hi - 1}", [r for r in rows if lo <= hand(r["sfen"]) < hi])
line("contested (|d16|<1500) & hand>=6", [r for r in rows if abs(r["d16"]) < 1500 and hand(r["sfen"]) >= 6])
