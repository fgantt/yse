# Our engine on one handicap-game position at several times, printing best move and score per depth.
# usage: python3 scripts/session37/probe_depths.py GAME.txt K1 K2 MS1,MS2,.. [THREADS] [NNUE]
# (our move K is played after moves[:2K-1]; YSE engine engines/usi-engine-stable)
import subprocess, sys, os, threading, queue
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from paths import REPO

G, k1, k2 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
times = [int(x) for x in sys.argv[4].split(",")]
threads = sys.argv[5] if len(sys.argv) > 5 else "4"
net = sys.argv[6] if len(sys.argv) > 6 else "nnue/default.bin"
lines = [l for l in open(G) if l.strip()]
start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
moves = [l for l in lines if not l.startswith("#")][0].split()
prefs = os.environ.get("SHOGI_PREFS_DIR", REPO + "/matches/s34_threads/prefs")
p = subprocess.Popen([REPO + "/engines/usi-engine-stable"], cwd=REPO, stdin=subprocess.PIPE,
                     stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1,
                     env={**os.environ, "SHOGI_PREFS_DIR": prefs})
q = queue.Queue(); threading.Thread(target=lambda: [q.put(l) for l in p.stdout], daemon=True).start()
def send(c): p.stdin.write(c + "\n"); p.stdin.flush()
def until(t):
    out = []
    while True:
        l = q.get(timeout=900); out.append(l)
        if l.startswith(t): return out
send("usi"); send(f"setoption name NNUEFile value {net}"); send(f"setoption name USI_Threads value {threads}")
send("isready"); until("readyok")
for k in range(k1, k2 + 1):
    for ms in times:
        send("usinewgame"); send("position " + start + " moves " + " ".join(moves[:2 * k - 1]))
        send(f"go btime 0 wtime 0 byoyomi {ms}"); out = until("bestmove")
        seen = {}
        for l in out:
            t = l.split()
            if l.startswith("info") and "depth" in t and "score" in t and "pv" in t and "bound" not in l:
                i = t.index("score"); seen[t[t.index("depth") + 1]] = (" ".join(t[i + 1:i + 3]), " ".join(t[t.index("pv") + 1:t.index("pv") + 6]), t[t.index("nodes") + 1] if "nodes" in t else "?")
        print(f"K{k} played {moves[2*k-1]} ours {ms/1000:g}s best {out[-1].split()[1]}", flush=True)
        for d, (sc, pv, n) in seen.items(): print(f"   d{d:>3} {sc:>12} n={n:>10} pv {pv}", flush=True)
send("quit")
