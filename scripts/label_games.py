"""Label every position of usi_match games with YaneuraOu (Session 35).

Turns match CSVs (start SFEN + `record` move list) into trainer JSONL: the positions our
engine actually reaches against YaneuraOu, which teacher self-play never visits.
    python3 scripts/label_games.py OUT.jsonl DEPTH WORKERS BOARD csv [csv ...]
BOARD is the start position's board field; the side to move is read off each game's first
move (a piece leaving ranks a-c is White's).
"""
import csv, json, os, subprocess, sys, threading, queue
from paths import require_yane
from multiprocessing import Pool
YANE = require_yane()
MATE = 30000

class Y:
    def __init__(self):
        self.p = subprocess.Popen([YANE], cwd=os.path.dirname(YANE), stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
        self.q = queue.Queue()
        threading.Thread(target=lambda: [self.q.put(l) for l in self.p.stdout], daemon=True).start()
        for c in ["usi", "setoption name USI_OwnBook value false", "setoption name BookFile value no_book",
                  "setoption name Threads value 1", "setoption name USI_Hash value 256", "isready"]:
            self.send(c)
        self.until("readyok"); self.send("usinewgame")
    def send(self, c): self.p.stdin.write(c + "\n"); self.p.stdin.flush()
    def until(self, t):
        out = []
        while True:
            l = self.q.get(timeout=600).strip(); out.append(l)
            if l.startswith(t): return out
    def label(self, pos, depth):
        self.send("position " + pos); self.send("d")
        sfen = next(l for l in self.until("sfen ") if l.startswith("sfen ")).split(" ", 1)[1]
        self.send(f"go depth {depth}"); out = self.until("bestmove"); sc = None
        for l in out:
            t = l.split()
            if t[:1] == ["info"] and "score" in t and "lowerbound" not in t and "upperbound" not in t:
                i = t.index("score"); v = t[i + 2]
                sc = int(v) if t[i + 1] == "cp" else (MATE - abs(int(v)) if not v.startswith("-") else -(MATE - abs(int(v))))
        return sfen, sc, out[-1].split()[1]

def work(args):
    games, depth = args
    y = Y(); rows = []
    for gid, moves, outcome, start in games:
        for ply in range(len(moves) + 1):
            pos = "sfen " + start + (" moves " + " ".join(moves[:ply]) if ply else "")
            sfen, sc, best = y.label(pos, depth)
            if sc is None or best in ("resign", "win"): break
            stm = sfen.split()[1]
            # Trainer format: sfen without the move number, eval from the side to move.
            rows.append({"game_id": gid, "ply": ply + 1, "sfen": " ".join(sfen.split()[:3]),
                         "player": "black" if stm == "b" else "white", "eval_cp": sc,
                         "outcome": outcome, "outcome_str": {1: "black_win", -1: "white_win", 0: "draw"}[outcome],
                         "from_random_opening": False, "random_move": False})
    y.send("quit")
    return rows

if __name__ == "__main__":
    out, depth, workers, board = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
    games = []; gid = 900000
    for f in sys.argv[5:]:
        for r in csv.DictReader(open(f)):
            moves = r["record"].split()
            res = r["result"]; a_black = r["a_color"] == "black"
            outcome = 0 if res not in ("AWin", "BWin") else (1 if (res == "AWin") == a_black else -1)
            if not moves: continue
            stm = "w" if moves[0][1] in "abc" else "b"
            games.append((gid, moves, outcome, f"{board} {stm} - 1")); gid += 1
    chunks = [(games[i::workers], depth) for i in range(workers)]
    n = 0
    with Pool(workers) as p, open(out, "w") as f:
        for rows in p.imap_unordered(work, chunks):
            for r in rows: f.write(json.dumps(r) + "\n")
            n += len(rows); print("labelled", n, flush=True)
    print(len(games), "games,", n, "positions")
