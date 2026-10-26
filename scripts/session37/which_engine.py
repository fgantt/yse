# Which engine played one side of a game? For each of that side's moves, ask each candidate for its
# best move (fresh process, Threads=1, fixed nodes) and count agreements.
# usage: python3 scripts/session37/which_engine.py GAME.txt SIDE(b|w) NODES1,NODES2
# Prints each candidate's match rate, then, for the moves where the candidates disagree, which one the game followed.
import subprocess, sys, os
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__)))); from paths import require_yane, REPO
G, side, budgets = sys.argv[1], sys.argv[2], [int(x) for x in sys.argv[3].split(",")]
S5 = os.path.join(os.path.dirname(REPO.rstrip("/")), "..", "shogi-engines", "yaneuraou-suisho5", "YaneuraOu-Suisho5")
S5 = os.path.normpath(S5) if os.path.exists(os.path.normpath(S5)) else os.path.normpath(os.path.join(REPO, "..", "..", "shogi-engines", "yaneuraou-suisho5", "YaneuraOu-Suisho5"))
cands = {"halfkpvm": (require_yane(), []), "suisho5": (S5, ["setoption name FV_SCALE value 24"])}
lines = [l for l in open(G) if l.strip()]
start = [l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
moves = [l for l in lines if not l.startswith("#")][0].split()
first = "w" if " w " in start else "b"
plies = [i for i in range(len(moves)) if (first if i % 2 == 0 else {"b": "w", "w": "b"}[first]) == side]
RES = {}
for name, (path, extra) in cands.items():
    assert os.path.exists(path), path
    for n in budgets:
        p = subprocess.Popen([path], cwd=os.path.dirname(path), stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
        def send(c): p.stdin.write(c + "\n"); p.stdin.flush()
        for c in ["usi", "setoption name Threads value 1", "setoption name BookFile value no_book", *extra, "isready"]: send(c)
        for l in p.stdout:
            if l.startswith("readyok"): break
        hit = 0; miss = []; picks = []
        for i in plies:
            send("usinewgame"); send("position " + start + (" moves " + " ".join(moves[:i]) if i else "")); send(f"go nodes {n}")
            for l in p.stdout:
                if l.startswith("bestmove"):
                    b = l.split()[1]; break
            picks.append(b)
            if b == moves[i]: hit += 1
            else: miss.append(i // 2 + 1)
        send("quit"); p.wait()
        RES[(name, n)] = picks
        print(f"{name:9s} {n/1e3:6.0f}k nodes: {hit}/{len(plies)} of the side's moves matched ({100*hit/len(plies):.0f}%)", flush=True)
# Moves where the candidates disagree with each other say which one played.
for n in budgets:
    a, b = RES[("halfkpvm", n)], RES[("suisho5", n)]
    diff = [k for k in range(len(plies)) if a[k] != b[k]]
    ha = sum(a[k] == moves[plies[k]] for k in diff); hb = sum(b[k] == moves[plies[k]] for k in diff)
    print(f"{n/1e3:6.0f}k: the two disagree on {len(diff)} moves; the game followed halfkpvm on {ha}, suisho5 on {hb}, neither on {len(diff)-ha-hb}")
