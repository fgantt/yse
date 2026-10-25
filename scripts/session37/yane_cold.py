# YaneuraOu (Threads=1, halfkpvm) score and best move per position with a fresh process each (cold
# hash), at several node budgets. Contrast: yane_scores.py keeps one process, so each position
# starts with the previous move's table, as in a game.
# usage: python3 scripts/session37/yane_cold.py NODES1,NODES2 GAME:K1-K2 [GAME:K1-K2 ..]
import subprocess, sys, os, json
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__)))); from paths import require_yane
Y = require_yane()
budgets = [int(x) for x in sys.argv[1].split(",")]
for spec in sys.argv[2:]:
    game, ks = spec.rsplit(":", 1); k1, k2 = map(int, ks.split("-"))
    lines = [l for l in open(game) if l.strip()]
    start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
    moves = [l for l in lines if not l.startswith("#")][0].split()
    for k in range(k1, k2 + 1):
        for n in budgets:
            cmds = ["usi", "setoption name Threads value 1", "setoption name BookFile value no_book", "isready",
                    "usinewgame", "position " + start + " moves " + " ".join(moves[:2 * k - 1]), f"go nodes {n}"]
            p = subprocess.Popen([Y], cwd=os.path.dirname(Y), stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
            p.stdin.write("\n".join(cmds) + "\n"); p.stdin.flush(); sc = None
            for l in p.stdout:
                t = l.split()
                if "score" in t and "bound" not in l:
                    i = t.index("score"); sc = int(t[i + 2]) if t[i + 1] == "cp" else (30000 if int(t[i + 2]) > 0 else -30000)
                if l.startswith("bestmove"):
                    best = t[1]; break
            p.stdin.write("quit\n"); p.stdin.flush(); p.wait()
            print(json.dumps({"game": os.path.basename(game), "k": k, "budget": n, "score": sc, "best": best}), flush=True)
