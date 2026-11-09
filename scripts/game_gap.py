"""Where do the user's lost games go wrong: evaluation or search?

Usage (from the repo root):
    python3 scripts/game_gap.py judge GAME...      # phase 1: score every one of our moves
    python3 scripts/game_gap.py ladder GAME...     # phase 2: node ladder on the costly moves
    python3 scripts/game_gap.py report GAME...
GAME is a record in matches/user_games/ without extension (e.g. 2026-11-01_even_loss172).
Output: matches/user_games/<GAME>_gap.jsonl (phase 1) and <GAME>_ladder.jsonl (phase 2).

Phase 1: for each of our moves, YaneuraOu (the default judge from scripts/paths.py,
1 thread, `go nodes JUDGE_NODES`) scores its own best move and, with `searchmoves`, the move
we played; loss = best - played, clamped to +-3000 cp. Unlike the older *_analysis.jsonl files
every move gets a score.

Phase 2: on moves that lost >= LADDER_MIN cp while the game was still open (|judge| <
OPEN), our engine (engines/usi-engine-stable, 1 thread, no book) and YaneuraOu each choose a
move at go nodes 25k, 100k, 400k, 1.6M, 6.4M; the judge scores each choice. A move whose loss
falls under 30 cp at some rung is "found at N nodes"; never found by our engine at 6.4M is
an evaluation-side miss (our net, or our search's shape, keeps preferring it). Node counts are
as each engine reports them; both count quiescence (ours since 849b44a), so equal numbers are
roughly equal work.
"""
import json, os, subprocess, sys, tempfile
from multiprocessing import Pool
from paths import require_yane

YANE = require_yane()
CAP = 3000


class Engine:  # as in move_loss.py, which runs its main() on import
    def __init__(self, path, cwd=None, options=()):
        self.p = subprocess.Popen([path], cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.send("usi"); self.until("usiok")
        for o in options: self.send(o)
        self.send("isready"); self.until("readyok"); self.send("usinewgame")

    def send(self, s): self.p.stdin.write(s + "\n"); self.p.stdin.flush()

    def until(self, tok):
        out = []
        while True:
            l = self.p.stdout.readline()
            if not l: raise SystemExit("engine died")
            l = l.strip(); out.append(l)
            if l.startswith(tok): return out

    def quit(self): self.send("quit")


def score_of(lines):
    s = None
    for l in lines:
        t = l.split()
        if t[:1] == ["info"] and "score" in t and "lowerbound" not in t and "upperbound" not in t:
            i = t.index("score")
            if t[i + 1] == "cp": s = int(t[i + 2])
            elif t[i + 1] == "mate": s = -CAP if t[i + 2].startswith("-") else CAP
    return max(-CAP, min(CAP, s if s is not None else 0))

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
GAMES = os.path.join(REPO, "matches/user_games")
OURS = os.path.join(REPO, "engines/usi-engine-stable")
JUDGE_NODES = int(os.environ.get("JUDGE_NODES", 3_000_000))
LADDER = [25_000, 100_000, 400_000, 1_600_000, 6_400_000]
LADDER_MIN, OPEN, FOUND = 40, 1500, 30
# Our engine persists `setoption USI_Threads` to its prefs file. Without SHOGI_PREFS_DIR that is
# the shared ~/Library/Application Support/shogi-vibe/engine_prefs.json, which the GUI reads too:
# on 2026-11-08 this script's `USI_Threads value 1` left every later GUI game on one thread.
# So the engine always gets a private prefs directory here.
PREFS = os.environ.get("SHOGI_PREFS_DIR") or os.path.join(tempfile.gettempdir(), "yse-game-gap-prefs")
YANE_OPTS = ["setoption name USI_OwnBook value false", "setoption name BookFile value no_book",
             "setoption name Threads value 1", "setoption name USI_Hash value 256"]
OUR_OPTS = ["setoption name USI_OwnBook value false", "setoption name USI_Threads value 1",
            "setoption name USI_Hash value 256"]


def load(game):
    lines = open(os.path.join(GAMES, game + ".txt")).read().splitlines()
    head = lines[0]
    moves = next(l for l in lines if l and not l.startswith("#")).split()
    i = head.find("YSE")
    side = 0 if "Black" in head[i:i + 60] or "Sente" in head[i:i + 60] else 1
    return moves, side


def pos(moves, ply):  # position before 1-based ply
    pre = moves[:ply - 1]
    return "startpos" + (" moves " + " ".join(pre) if pre else "")


class Eng(Engine):
    def go(self, position, go):
        self.send("position " + position); self.send("go " + go)
        lines = self.until("bestmove")
        return lines[-1].split()[1], lines


def judge_engine():
    return Eng(YANE, cwd=YANE.rsplit("/", 1)[0], options=YANE_OPTS)


def judge_move(j, p, mv, cache):
    if mv not in cache:
        if mv in ("resign", "win", "none"):
            cache[mv] = -CAP
        else:
            _, l = j.go(p, f"nodes {JUDGE_NODES} searchmoves {mv}")
            cache[mv] = score_of(l)
    return cache[mv]


def phase_judge(game):
    moves, side = load(game)
    j = judge_engine()
    out = open(os.path.join(GAMES, game + "_gap.jsonl"), "w")
    for ply in range(1, len(moves) + 1):
        if (ply - 1) % 2 != side:
            continue
        p = pos(moves, ply)
        best, l = j.go(p, f"nodes {JUDGE_NODES}")
        cache = {best: score_of(l)}
        played = judge_move(j, p, moves[ply - 1], cache)
        out.write(json.dumps({"ply": ply, "played": moves[ply - 1], "best": best,
                              "best_cp": cache[best], "played_cp": played,
                              "loss": max(0, cache[best] - played)}) + "\n")
        out.flush()
    j.quit()
    return game


def phase_ladder(game):
    moves, _ = load(game)
    rows = [json.loads(l) for l in open(os.path.join(GAMES, game + "_gap.jsonl"))]
    pick = [r for r in rows if r["loss"] >= LADDER_MIN and abs(r["best_cp"]) < OPEN]
    j = judge_engine()
    env_prefs = {"SHOGI_PREFS_DIR": PREFS}
    os.environ.update(env_prefs)
    ours = Eng(OURS, cwd=REPO, options=OUR_OPTS)
    yane = Eng(YANE, cwd=YANE.rsplit("/", 1)[0], options=YANE_OPTS)
    out = open(os.path.join(GAMES, game + "_ladder.jsonl"), "w")
    for r in pick:
        p = pos(moves, r["ply"])
        cache = {r["best"]: r["best_cp"], r["played"]: r["played_cp"]}
        res = {"ply": r["ply"], "played": r["played"], "best": r["best"], "loss": r["loss"],
               "best_cp": r["best_cp"]}
        for name, e in (("ours", ours), ("yane", yane)):
            rung = []
            for n in LADDER:
                e.send("usinewgame")
                mv, _ = e.go(p, f"nodes {n}")
                rung.append([n, mv, max(0, r["best_cp"] - judge_move(j, p, mv, cache))])
            res[name] = rung
        out.write(json.dumps(res) + "\n"); out.flush()
    for e in (j, ours, yane):
        e.quit()
    return game


def report(games):
    def found(rung):
        return next((n for n, _, loss in rung if loss < FOUND), None)
    for g in games:
        rows = [json.loads(l) for l in open(os.path.join(GAMES, g + "_gap.jsonl"))]
        openr = [r for r in rows if abs(r["best_cp"]) < OPEN]
        first = next((r["ply"] for r in rows if r["best_cp"] < -300), None)
        L = [r["loss"] for r in openr]
        print(f"{g}: our moves {len(rows)}, open {len(openr)}, judge < -300 from ply {first}; "
              f"open-phase loss sum {sum(L)}, mean {sum(L) / max(1, len(L)):.0f}, "
              f">=40: {sum(x >= 40 for x in L)}, >=100: {sum(x >= 100 for x in L)}, "
              f"best move {100 * sum(r['played'] == r['best'] for r in openr) / max(1, len(openr)):.0f}%")
        lp = os.path.join(GAMES, g + "_ladder.jsonl")
        if os.path.exists(lp):
            for r in map(json.loads, open(lp)):
                fo, fy = found(r["ours"]), found(r["yane"])
                print(f"  ply {r['ply']:3} {r['played']:6} loss {r['loss']:4} (judge {r['best_cp']:+5}) "
                      f"ours found {fo or '-':>8}  yane found {fy or '-':>8}  "
                      f"ours@6.4M {r['ours'][-1][1]} ({r['ours'][-1][2]})")


if __name__ == "__main__":
    cmd, games = sys.argv[1], sys.argv[2:]
    if cmd == "report":
        report(games)
    else:
        fn = {"judge": phase_judge, "ladder": phase_ladder}[cmd]
        with Pool(len(games)) as pool:
            for g in pool.imap_unordered(fn, games):
                print("done", g, flush=True)
