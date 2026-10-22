# One handicap-game position at fixed depths (1 thread) under each search env toggle.
# usage: python3 scripts/session37/toggle_probe.py GAME.txt K D1,D2,.. [TOGGLE,..]
import subprocess, sys, os, threading, queue, time
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from paths import REPO
G, k, depths = sys.argv[1], int(sys.argv[2]), sys.argv[3].split(",")
toggles = sys.argv[4].split(",") if len(sys.argv) > 4 else ["", "SHOGI_NO_LMR", "SHOGI_NO_PRUNING", "SHOGI_NO_NULLMOVE", "SHOGI_NO_SEE_QUIET"]
lines = [l for l in open(G) if l.strip()]
start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
moves = [l for l in lines if not l.startswith("#")][0].split()
pos = start + " moves " + " ".join(moves[:2 * k - 1])
for tg in toggles:
    env = {**os.environ, "SHOGI_PREFS_DIR": os.environ.get("SHOGI_PREFS_DIR", REPO + "/matches/s34_threads/prefs")}
    if tg: env[tg] = "1"
    p = subprocess.Popen([REPO + "/engines/usi-engine-stable"], cwd=REPO, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                         stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env)
    q = queue.Queue(); threading.Thread(target=lambda: [q.put(l) for l in p.stdout], daemon=True).start()
    def send(c): p.stdin.write(c + "\n"); p.stdin.flush()
    def until(t):
        out = []
        while True:
            l = q.get(timeout=1800); out.append(l)
            if l.startswith(t): return out
    send("usi"); send("setoption name USI_Threads value 1"); send("isready"); until("readyok")
    row = []
    for d in depths:
        t0 = time.time(); send("usinewgame"); send("position " + pos); send(f"go depth {d}"); out = until("bestmove"); sc = "?"; n = "?"
        for l in out:
            t = l.split()
            if "score" in t and "bound" not in l:
                i = t.index("score"); sc = t[i + 2] if t[i + 1] == "cp" else "M" + t[i + 2]
                n = t[t.index("nodes") + 1] if "nodes" in t else n
        row.append(f"d{d}:{sc}/{out[-1].split()[1]}({float(n)/1e6:.1f}M,{time.time()-t0:.0f}s)" if n != "?" else f"d{d}:{sc}/{out[-1].split()[1]}")
    print(f"K{k} {tg or 'baseline':<20}", " ".join(row), flush=True)
    send("quit"); p.wait()
