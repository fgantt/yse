# YaneuraOu (Threads=1) score per depth before our moves K1..K2 of a handicap game.
# usage: python3 scripts/session37/yane_depths.py GAME.txt K1 K2 [NODES]
import subprocess, sys, os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__)))); from paths import require_yane
Y = require_yane()
G, k1, k2 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
nodes = sys.argv[4] if len(sys.argv) > 4 else "2000000"
lines = [l for l in open(G) if l.strip()]
start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
moves = [l for l in lines if not l.startswith("#")][0].split()
p = subprocess.Popen([Y], cwd=os.path.dirname(Y), stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
def send(c): p.stdin.write(c + "\n"); p.stdin.flush()
for c in ["usi", "setoption name Threads value 1", "setoption name BookFile value no_book", "isready"]: send(c)
for l in p.stdout:
    if l.startswith("readyok"): break
for k in range(k1, k2 + 1):
    send("usinewgame"); send("position " + start + " moves " + " ".join(moves[:2 * k - 1])); send(f"go nodes {nodes}")
    print(f"K{k} played {moves[2*k-1]}", flush=True)
    for l in p.stdout:
        t = l.split()
        if "score" in t and "pv" in t and "bound" not in l and "depth" in t:
            i = t.index("score")
            print(f"   d{t[t.index('depth')+1]:>3} {' '.join(t[i+1:i+3]):>12} n={t[t.index('nodes')+1]:>9} pv {' '.join(t[t.index('pv')+1:t.index('pv')+6])}", flush=True)
        if l.startswith("bestmove"): print("  best", t[1], flush=True); break
send("quit")
