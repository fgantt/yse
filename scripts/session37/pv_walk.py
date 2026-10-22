# Walk a line of moves from one handicap-game position; at each step print our shallow scores
# (go depth 1 = quiescence-level view) and a fixed-depth score, from the side to move after the step.
# usage: python3 scripts/session37/pv_walk.py GAME.txt K "m1 m2 ..." [DEPTH] [NNUE]
import subprocess, sys, os, threading, queue
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from paths import REPO

G, k, line = sys.argv[1], int(sys.argv[2]), sys.argv[3].split()
depth = sys.argv[4] if len(sys.argv) > 4 else "10"
net = sys.argv[5] if len(sys.argv) > 5 else "nnue/default.bin"
lines = [l for l in open(G) if l.strip()]
start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
moves = [l for l in lines if not l.startswith("#")][0].split()
p = subprocess.Popen([REPO + "/engines/usi-engine-stable"], cwd=REPO, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1,
                     env={**os.environ, "SHOGI_PREFS_DIR": os.environ.get("SHOGI_PREFS_DIR", REPO + "/matches/s34_threads/prefs")})
q = queue.Queue(); threading.Thread(target=lambda: [q.put(l) for l in p.stdout], daemon=True).start()
def send(c): p.stdin.write(c + "\n"); p.stdin.flush()
def until(t):
    out = []
    while True:
        l = q.get(timeout=900); out.append(l)
        if l.startswith(t): return out
def go(pos, d):
    send("usinewgame"); send("position " + pos); send(f"go depth {d}"); out = until("bestmove"); last = "?"
    for l in out:
        t = l.split()
        if "score" in t and "bound" not in l:
            i = t.index("score"); last = " ".join(t[i + 1:i + 3]) + " pv " + " ".join(t[t.index("pv") + 1:t.index("pv") + 5]) if "pv" in t else ""
    return last
send("usi"); send(f"setoption name NNUEFile value {net}"); send("setoption name USI_Threads value 1")
send("isready"); until("readyok")
base = moves[:2 * k - 1]
for i in range(len(line) + 1):
    pos = start + " moves " + " ".join(base + line[:i])
    who = "us" if i % 2 == 0 else "them"
    print(f"after {' '.join(line[:i]) or '(start)':<30} to move {who:<4} d1 {go(pos, 1):<40} d{depth} {go(pos, depth)}", flush=True)
send("quit")
