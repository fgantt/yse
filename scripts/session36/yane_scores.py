# YaneuraOu (Threads=1, 2M nodes) score, best move and PV before each of our moves K1..K2 of a saved game.
# usage: python3 scripts/session36/yane_scores.py GAME.txt K1 K2  (our move K is ply 2K of a handicap game: moves[:2K-1])
import subprocess, sys, os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__)))); from paths import require_yane

Y = require_yane()
G, k1, k2 = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
lines = [l for l in open(G) if l.strip()]
start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
moves = [l for l in lines if not l.startswith("#")][0].split()
cmds = ["usi", "setoption name Threads value 1", "setoption name BookFile value no_book", "isready", "usinewgame"]
for k in range(k1, k2 + 1):
    cmds += ["position " + start + " moves " + " ".join(moves[:2 * k - 1]), "go nodes 2000000"]
cmds.append("quit")
p = subprocess.Popen([Y], cwd=os.path.dirname(Y), stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
# send lazily: each go must finish before the next position
p.stdin.write("\n".join(cmds[:5]) + "\n"); p.stdin.flush()
rest = cmds[5:]; last = None; k = k1
while rest:
    p.stdin.write(rest.pop(0) + "\n" + rest.pop(0) + "\n"); p.stdin.flush()
    for l in p.stdout:
        t = l.split()
        if "score" in t and "lowerbound" not in t and "upperbound" not in t:
            i = t.index("score"); last = " ".join(t[i+1:i+3]); pv = t[t.index("pv")+1:t.index("pv")+6] if "pv" in t else []
        if l.startswith("bestmove"):
            print(k, moves[2 * k - 1], "yane", last, "best", l.split()[1], " ".join(pv)); k += 1; break
    if rest == ["quit"]: break
