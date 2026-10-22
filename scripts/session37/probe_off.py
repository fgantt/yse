# Score and best move at fixed node budgets (1 thread) with lean-search rules switched off.
# usage: python3 scripts/session37/probe_off.py ENGINE OFF NODES1,NODES2 GAME:K1-K2 [GAME:K1-K2 ..]
#   OFF is a SHOGI_LEAN_OFF value ("-" = none). Prints one JSON line per position and budget.
import subprocess, sys, os, threading, queue, time, json
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from paths import REPO
engine, off, budgets = sys.argv[1], sys.argv[2], [int(x) for x in sys.argv[3].split(",")]
env = {**os.environ, "SHOGI_PREFS_DIR": os.environ.get("SHOGI_PREFS_DIR", REPO + "/matches/s34_threads/prefs")}
if off != "-": env["SHOGI_LEAN_OFF"] = off
p = subprocess.Popen([engine], cwd=REPO, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                     stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env)
q = queue.Queue(); threading.Thread(target=lambda: [q.put(l) for l in p.stdout], daemon=True).start()
def send(c): p.stdin.write(c + "\n"); p.stdin.flush()
def until(t):
    out = []
    while True:
        l = q.get(timeout=3600); out.append(l)
        if l.startswith(t): return out
send("usi"); send("setoption name USI_Threads value 1"); send("isready"); until("readyok")
for spec in sys.argv[4:]:
    game, ks = spec.rsplit(":", 1); k1, k2 = map(int, ks.split("-"))
    lines = [l for l in open(game) if l.strip()]
    start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
    moves = [l for l in lines if not l.startswith("#")][0].split()
    for k in range(k1, k2 + 1):
        for n in budgets:
            t0 = time.time(); send("usinewgame"); send("position " + start + " moves " + " ".join(moves[:2 * k - 1]))
            send(f"go nodes {n}"); out = until("bestmove"); r = {}
            for l in out:
                t = l.split()
                if "score" in t and "bound" not in l and "depth" in t:
                    i = t.index("score")
                    r = {"score": int(t[i + 2]) if t[i + 1] == "cp" else (30000 if int(t[i + 2]) > 0 else -30000),
                         "depth": int(t[t.index("depth") + 1]), "nodes": int(t[t.index("nodes") + 1]) if "nodes" in t else None}
            print(json.dumps({"off": off, "game": os.path.basename(game), "k": k, "budget": n,
                              "best": out[-1].split()[1], "secs": round(time.time() - t0, 1), **r}), flush=True)
send("quit")
