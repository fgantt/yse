"""Turning-point test set from the user's GUI four-piece games (Session 35).

judge: every Black (our) move of every game in matches/user_games/*4mai*.txt is judged by
  YaneuraOu (best move and score; the played move's score by searching the position after it,
  negated -- `searchmoves` ignores drops). Output: one JSON row per position.
    python3 scripts/loss_set.py judge OUT.jsonl NODES THREADS [GAME.txt]
  (with GAME, only that game is judged and its rows are appended to OUT)
score: an engine plays each selected position at a fixed byoyomi (MS), or with MS 0 at the
  clock it would have in the GUI at that move (60 min + 10 s); its moves are judged the same
  way (cached in the judge file's rows).
    python3 scripts/loss_set.py score JUDGE.jsonl ENGINE MS OUT.jsonl [reps]
"""
import json, os, re, subprocess, sys, threading, queue
REPO = "/Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue"
YANE = "/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1"
START = "1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
MOVE = re.compile(r"^([1-9][a-i][1-9][a-i]\+?|[PLNSGBR]\*[1-9][a-i])$")
CAP = 30000

def load_game(path):
    lines = [l for l in open(path) if l.strip()]
    st = [l for l in lines if l.startswith("# start:")]
    start = st[0].split("start:")[1].strip().replace("sfen ", "") if st else START
    moves = []
    for l in lines:
        if l.startswith("#"): continue
        moves += [t for t in l.split() if MOVE.match(t)]
    return start, moves

class Engine:
    def __init__(self, path, cwd, opts, env=None):
        self.p = subprocess.Popen([path], cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1,
                                  env={**os.environ, **(env or {})})
        self.q = queue.Queue()
        threading.Thread(target=lambda: [self.q.put(l) for l in self.p.stdout], daemon=True).start()
        self.send("usi")
        for k, v in opts: self.send(f"setoption name {k} value {v}")
        self.send("isready"); self.until("readyok"); self.send("usinewgame")
    def send(self, c): self.p.stdin.write(c + "\n"); self.p.stdin.flush()
    def until(self, t):
        out = []
        while True:
            l = self.q.get(timeout=900).strip(); out.append(l)
            if l.startswith(t): return out
    def go(self, pos, go):
        self.send("position " + pos); self.send("go " + go)
        out = self.until("bestmove"); sc = None; depth = None; bound = None
        for l in out:
            t = l.split()
            if t[:1] == ["info"] and "score" in t:
                i = t.index("score"); v = t[i + 2]
                v = int(v) if t[i + 1] == "cp" else (-CAP if v.startswith("-") else CAP)
                if "lowerbound" in t or "upperbound" in t:
                    bound = v; continue
                sc = v
                if "depth" in t: depth = int(t[t.index("depth") + 1])
        # YaneuraOu may print only its final iteration, and that line can be a bound;
        # use the bound then rather than report no score.
        return out[-1].split()[1], sc if sc is not None else bound, depth
    def quit(self): self.send("quit")

def yane(threads):
    return Engine(YANE, os.path.dirname(YANE), [("USI_OwnBook", "false"), ("BookFile", "no_book"),
                  ("Threads", str(threads)), ("USI_Hash", "2048")])

def judge_move(y, pos, mv, nodes):
    if mv in ("resign", "win", "none"): return -CAP
    _, sc, _ = y.go(pos + " " + mv if " moves " in pos else pos + " moves " + mv, f"nodes {nodes}")
    return -(sc if sc is not None else 0)

def judge(out, nodes, threads, game=None):
    y = yane(threads)
    games = [game] if game else sorted(
        f for f in os.listdir(REPO + "/matches/user_games") if "4mai" in f and f.endswith(".txt"))
    with open(out, "a" if game else "w") as f:
        for g in games:
            start, moves = load_game(REPO + "/matches/user_games/" + g)
            first = start.split()[1]
            for i, mv in enumerate(moves):
                side = first if i % 2 == 0 else ("b" if first == "w" else "w")
                if side != "b": continue
                pos = "sfen " + start + (" moves " + " ".join(moves[:i]) if i else "")
                best, bsc, d = y.go(pos, f"nodes {nodes}")
                psc = bsc if mv == best else judge_move(y, pos, mv, nodes)
                r = {"game": g, "ply": i + 1, "move_no": (i + 2) // 2, "pos": pos, "played": mv,
                     "best": best, "best_score": bsc, "played_score": psc, "depth": d,
                     "loss": (bsc or 0) - (psc or 0), "scores": {best: bsc, mv: psc}}
                f.write(json.dumps(r) + "\n"); f.flush()
                print(g, r["move_no"], mv, best, bsc, psc, r["loss"], flush=True)
    y.quit()

def score(judge_file, engine, ms, out, reps=1, nodes=30000000, threads=12):
    rows = [json.loads(l) for l in open(judge_file)]
    sel = [r for r in rows if r.get("selected")]
    y = yane(threads)
    env = {"SHOGI_PREFS_DIR": os.environ.get("SHOGI_PREFS_DIR", "")}
    with open(out, "w") as f:
        for r in sel:
            for k in range(reps):
                e = Engine(engine, REPO, [("NNUEFile", "nnue/default.bin")], env)
                # ms 0: the clock the GUI would show at this move (60 min + 10 s byoyomi,
                # our allocation of remaining/30 + byoyomi each move before it).
                if ms == 0:
                    rem = int(3600000 * (29 / 30) ** (r["move_no"] - 1))
                    go = f"btime {rem} wtime {rem} byoyomi 10000"
                else:
                    go = f"btime 0 wtime 0 byoyomi {ms}"
                mv, sc, d = e.go(r["pos"], go)
                e.quit()
                if mv not in r["scores"]:
                    r["scores"][mv] = judge_move(y, r["pos"], mv, nodes)
                loss = r["best_score"] - r["scores"][mv]
                o = {"game": r["game"], "move_no": r["move_no"], "rep": k, "move": mv, "our_score": sc,
                     "depth": d, "loss": loss, "played": r["played"], "best": r["best"]}
                f.write(json.dumps(o) + "\n"); f.flush()
                print(r["game"], r["move_no"], mv, loss, flush=True)
    y.quit()
    # Cache the newly judged moves back into the judge file.
    allrows = {(r["game"], r["ply"]): r for r in rows}
    for r in sel: allrows[(r["game"], r["ply"])] = r
    with open(judge_file, "w") as f:
        for r in rows: f.write(json.dumps(allrows[(r["game"], r["ply"])]) + "\n")

def gui_alloc_ms(move_no):
    """What our engine spends on this move in the GUI (60 min + 10 s byoyomi, 1/30 of the
    remaining main time + byoyomi each move)."""
    return int(3600000 * (29 / 30) ** (move_no - 1)) // 30 + 10000

def select(judge_file, out, n_controls=30, seed=35):
    """Mistakes: loss >= 250 cp while the game is open (-1500 < best < 8000). Controls: moves
    15-60 in open positions where we matched YaneuraOu within 30 cp."""
    import random
    rows = [json.loads(l) for l in open(judge_file)]
    ok = [r for r in rows if r["best_score"] is not None and r["played_score"] is not None
          and -1500 < r["best_score"] < 8000]
    mistakes = [r for r in ok if r["loss"] >= 250]
    pool = [r for r in ok if r["loss"] <= 30 and 15 <= r["move_no"] <= 60]
    random.Random(seed).shuffle(pool)
    with open(out, "w") as f:
        for r in mistakes: f.write(json.dumps({**r, "kind": "mistake"}) + "\n")
        for r in pool[:n_controls]: f.write(json.dumps({**r, "kind": "control"}) + "\n")
    print(len(mistakes), "mistakes,", min(n_controls, len(pool)), "controls")

def trace(set_file, engine, mult, out):
    """Search each position for `mult` x its GUI allocation; record every iteration's move and
    time, so the move chosen at any time up to that is known."""
    rows = [json.loads(l) for l in open(set_file)]
    env = {"SHOGI_PREFS_DIR": os.environ.get("SHOGI_PREFS_DIR", "")}
    with open(out, "w") as f:
        for r in rows:
            e = Engine(engine, REPO, [("NNUEFile", "nnue/default.bin")], env)
            e.send("position " + r["pos"])
            e.send(f"go btime 0 wtime 0 byoyomi {int(gui_alloc_ms(r['move_no']) * mult)}")
            lines = e.until("bestmove"); e.quit()
            it = []
            for l in lines:
                t = l.split()
                if t[:1] == ["info"] and "pv" in t and "time" in t and "lowerbound" not in t and "upperbound" not in t:
                    it.append((int(t[t.index("time") + 1]), int(t[t.index("depth") + 1]), t[t.index("pv") + 1]))
            o = {"game": r["game"], "ply": r["ply"], "move_no": r["move_no"], "kind": r["kind"],
                 "gui_ms": gui_alloc_ms(r["move_no"]), "iters": it, "bestmove": lines[-1].split()[1]}
            f.write(json.dumps(o) + "\n"); f.flush()
            print(r["game"], r["move_no"], r["kind"], o["bestmove"], it[-1][:2] if it else None, flush=True)

def move_at(tr, ms):
    m = None
    for t, d, mv in tr["iters"]:
        if t <= ms: m = mv
    return m or tr["bestmove"]

def report(set_file, trace_file, mults, nodes=20000000, threads=16):
    """Judge the traced engine's move at each multiple of the GUI allocation."""
    rows = {(r["game"], r["ply"]): r for r in map(json.loads, open(set_file))}
    trs = [json.loads(l) for l in open(trace_file)]
    y = None
    for tr in trs:
        r = rows[(tr["game"], tr["ply"])]
        for m in mults:
            mv = tr["bestmove"] if m == max(mults) else move_at(tr, tr["gui_ms"] * m)
            if mv not in r["scores"]:
                y = y or yane(threads)
                r["scores"][mv] = judge_move(y, r["pos"], mv, nodes)
    if y: y.quit()
    with open(set_file, "w") as f:
        for r in rows.values(): f.write(json.dumps(r) + "\n")
    for kind in ("mistake", "control"):
        sel = [tr for tr in trs if tr["kind"] == kind]
        if not sel: continue
        print(f"{kind}s ({len(sel)}):")
        for m in mults:
            losses = []
            for tr in sel:
                r = rows[(tr["game"], tr["ply"])]
                mv = tr["bestmove"] if m == max(mults) else move_at(tr, tr["gui_ms"] * m)
                losses.append(max(0, r["best_score"] - r["scores"][mv]))
            print(f"  {m}x GUI time: mean loss {sum(losses)/len(losses):.0f}, "
                  f">=250: {sum(l >= 250 for l in losses)}, best-or-near (<=30): {sum(l <= 30 for l in losses)}")

def yane_budget(set_file, budgets, threads=1, nodes=20000000):
    """YaneuraOu's own move at small node budgets on the same positions: if a shallow
    YaneuraOu finds what our deep search misses, its evaluation does the work."""
    rows = [json.loads(l) for l in open(set_file)]
    yb = yane(threads); judge_e = None
    res = {b: {"mistake": [], "control": []} for b in budgets}
    for r in rows:
        for b in budgets:
            mv, _, d = yb.go(r["pos"], f"nodes {b}")
            if mv not in r["scores"]:
                judge_e = judge_e or yane(16)
                r["scores"][mv] = judge_move(judge_e, r["pos"], mv, nodes)
            res[b][r["kind"]].append(max(0, r["best_score"] - r["scores"][mv]))
    yb.quit()
    if judge_e: judge_e.quit()
    with open(set_file, "w") as f:
        for r in rows: f.write(json.dumps(r) + "\n")
    for b in budgets:
        for k, L in res[b].items():
            print(f"YaneuraOu {b:>9} nodes, {k}s ({len(L)}): mean loss {sum(L)/len(L):.0f}, "
                  f">=250: {sum(l >= 250 for l in L)}, <=30: {sum(l <= 30 for l in L)}", flush=True)

def _fixed_one(args):
    engine, nnue, depth, r = args
    S = os.environ["LOSS_SET_PREFS1"]
    e = Engine(engine, REPO, [("NNUEFile", nnue)], {"SHOGI_PREFS_DIR": S})
    mv, sc, d = e.go(r["pos"], f"depth {depth}")
    e.quit()
    return (r["game"], r["ply"]), mv

def fixed(set_file, engine, nnue, depth, out, workers=12, nodes=20000000):
    """Every position at a fixed depth, one thread per engine, many in parallel: deterministic,
    so two networks compare on equal terms in minutes."""
    from multiprocessing import Pool
    rows = [json.loads(l) for l in open(set_file)]
    with Pool(workers) as p:
        moves = dict(p.map(_fixed_one, [(engine, nnue, depth, r) for r in rows]))
    y = None
    res = {"mistake": [], "control": []}
    with open(out, "w") as f:
        for r in rows:
            mv = moves[(r["game"], r["ply"])]
            if mv not in r["scores"]:
                y = y or yane(16)
                r["scores"][mv] = judge_move(y, r["pos"], mv, nodes)
            loss = max(0, r["best_score"] - r["scores"][mv])
            res[r["kind"]].append(loss)
            f.write(json.dumps({"game": r["game"], "ply": r["ply"], "kind": r["kind"], "move": mv, "loss": loss}) + "\n")
    if y: y.quit()
    with open(set_file, "w") as f:
        for r in rows: f.write(json.dumps(r) + "\n")
    for k, L in res.items():
        print(f"{nnue} depth {depth}, {k}s ({len(L)}): mean loss {sum(L)/len(L):.0f}, "
              f">=250: {sum(l >= 250 for l in L)}, <=30: {sum(l <= 30 for l in L)}")

if __name__ == "__main__":
    if sys.argv[1] == "fixed": fixed(sys.argv[2], sys.argv[3], sys.argv[4], int(sys.argv[5]), sys.argv[6]); sys.exit()
    if sys.argv[1] == "yane": yane_budget(sys.argv[2], [int(x) for x in sys.argv[3].split(",")]); sys.exit()
    if sys.argv[1] == "select": select(sys.argv[2], sys.argv[3]); sys.exit()
    if sys.argv[1] == "trace": trace(sys.argv[2], sys.argv[3], float(sys.argv[4]), sys.argv[5]); sys.exit()
    if sys.argv[1] == "report": report(sys.argv[2], sys.argv[3], [float(x) for x in sys.argv[4].split(",")]); sys.exit()
    if sys.argv[1] == "judge": judge(sys.argv[2], int(sys.argv[3]), int(sys.argv[4]),
                                        sys.argv[5] if len(sys.argv) > 5 else None)
    else: score(sys.argv[2], sys.argv[3], int(sys.argv[4]), sys.argv[5], int(sys.argv[6]) if len(sys.argv) > 6 else 1)
