# Per-iteration score lines (depth >= 10) for one handicap-game position at a node budget, 1 thread.
# usage: python3 scripts/session37/score_trace.py ENGINE GAME K NODES
import subprocess, os, sys
eng, G, k, n = sys.argv[1], sys.argv[2], int(sys.argv[3]), sys.argv[4]
lines = [l for l in open(G) if l.strip()]
start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
moves = [l for l in lines if not l.startswith("#")][0].split()
env = {**os.environ, "SHOGI_PREFS_DIR": os.environ.get("SHOGI_PREFS_DIR", "matches/s34_threads/prefs")}
p = subprocess.Popen([eng], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, env=env)
for c in ["usi", "setoption name USI_Threads value 1", "isready", "usinewgame",
          "position " + start + " moves " + " ".join(moves[:2 * k - 1]), f"go nodes {n}"]:
    p.stdin.write(c + "\n")
p.stdin.flush()
for l in p.stdout:
    t = l.split()
    if l.startswith("info depth") and "score" in t and int(t[2]) >= 10:
        i = t.index("score"); print(f"d{t[2]} {t[i+1]} {t[i+2]} nodes {t[t.index('nodes')+1]} pv {' '.join(t[t.index('pv')+1:])}")
    if l.startswith("bestmove"): print(l.strip()); break
p.stdin.write("quit\n"); p.stdin.flush(); p.wait()
