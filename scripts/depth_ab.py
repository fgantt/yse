"""Depth and nodes reached by two USI engines at a fixed time per move, alternating per position.

Usage (from the repo root):
    python3 scripts/depth_ab.py <engine old> <engine new> nnue_corpus_yaneura_d10.jsonl <n> [byoyomi_ms]

Positions: every 2011th corpus record (the `search_bench` stride), at most n;
the d10 corpus yields 34. Default engine options, so parallel search, as shipped;
byoyomi_ms defaults to 1000.
"""
import subprocess, sys, json, statistics, time
engines = {"old": sys.argv[1], "new": sys.argv[2]}
corpus, n, stride = sys.argv[3], int(sys.argv[4]), 2011
byoyomi = int(sys.argv[5]) if len(sys.argv) > 5 else 1000
sfens = []
for i, l in enumerate(open(corpus)):
    if i % stride == 0:
        sfens.append(json.loads(l)["sfen"])
    if len(sfens) >= n: break
procs = {}
for k, path in engines.items():
    p = subprocess.Popen([path], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
    procs[k] = p
def send(p, s): p.stdin.write(s + "\n"); p.stdin.flush()
def until(p, tok):
    out = []
    while True:
        l = p.stdout.readline()
        if not l: raise SystemExit("engine died")
        out.append(l.strip())
        if l.startswith(tok): return out
for p in procs.values():
    send(p, "usi"); until(p, "usiok"); send(p, "isready"); until(p, "readyok"); send(p, "usinewgame")
res = {k: [] for k in engines}
for idx, sf in enumerate(sfens):
    for k, p in procs.items():
        send(p, "position sfen " + sf); send(p, f"go btime 0 wtime 0 byoyomi {byoyomi}")
        t0 = time.time(); lines = until(p, "bestmove"); dt = time.time() - t0
        depth = nodes = 0
        for l in lines:
            t = l.split()
            if t[:1] == ["info"] and "depth" in t:
                depth = int(t[t.index("depth") + 1])
                if "nodes" in t: nodes = int(t[t.index("nodes") + 1])
        res[k].append((depth, nodes, dt))
    print(idx, " ".join(f"{k}: d{res[k][-1][0]} n{res[k][-1][1]}" for k in engines), flush=True)
for k in engines:
    d = [r[0] for r in res[k]]; nn = [r[1] for r in res[k]]; t = [r[2] for r in res[k]]
    print(f"{k}: mean depth {statistics.mean(d):.2f} median {statistics.median(d)} | median nodes {statistics.median(nn):.0f} | mean time {statistics.mean(t):.2f}s")
diff = [a[0] - b[0] for a, b in zip(res["new"], res["old"])]
print(f"new-old depth: mean {statistics.mean(diff):+.2f}, deeper {sum(x>0 for x in diff)}, same {sum(x==0 for x in diff)}, shallower {sum(x<0 for x in diff)}")
for p in procs.values(): send(p, "quit")
