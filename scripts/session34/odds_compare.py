# Compare our engine with YaneuraOu on the odds-holder's moves of YaneuraOu-vs-YaneuraOu games.
#   python3 odds_compare.py OUT.jsonl WORKER NWORKERS csv...
# For each position (move >= MIN_MOVE, odds side to move): YaneuraOu's game move, our move
# (1 s, 4 threads), and a judge (YaneuraOu 3M nodes) scoring best, Yane's move and ours.
import csv, json, os, subprocess, sys, threading, queue

REPO = "/Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue"
YANE = "/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1"
START = "1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
MIN_MOVE, MAX_PER_GAME = 15, 25
CAP = 3000


class Engine:
    def __init__(self, path, cwd, opts):
        env = {**os.environ, "SHOGI_PREFS_DIR": REPO + "/matches/s34_threads/prefs"}
        self.p = subprocess.Popen([path], cwd=cwd, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                  stderr=subprocess.DEVNULL, text=True, bufsize=1, env=env)
        self.q = queue.Queue()
        threading.Thread(target=lambda: [self.q.put(l) for l in self.p.stdout], daemon=True).start()
        self.send("usi")
        for k, v in opts:
            self.send(f"setoption name {k} value {v}")
        self.send("isready"); self.until("readyok"); self.send("usinewgame")

    def send(self, c):
        self.p.stdin.write(c + "\n"); self.p.stdin.flush()

    def until(self, tok):
        out = []
        while True:
            l = self.q.get(timeout=900); out.append(l)
            if l.startswith(tok):
                return out

    def go(self, pos, go):
        self.send("position " + pos); self.send("go " + go)
        out = self.until("bestmove")
        sc = None
        for l in out:
            t = l.split()
            if t[:2] == ["info", "depth"] and "score" in t and "lowerbound" not in t and "upperbound" not in t:
                i = t.index("score")
                sc = int(t[i + 2]) if t[i + 1] == "cp" else (CAP if not t[i + 2].startswith("-") else -CAP)
        return out[-1].split()[1], max(-CAP, min(CAP, sc if sc is not None else 0))


def main():
    out_path, worker, nworkers = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    games = []
    for f in sys.argv[4:]:
        games += [r["record"].split() for r in csv.DictReader(open(f))]
    games = games[worker::nworkers]
    ours = Engine(REPO + "/engines/usi-engine-stable", REPO,
                  [("NNUEFile", "nnue/default.bin"), ("USI_Threads", "4")])
    judge = Engine(YANE, os.path.dirname(YANE),
                   [("USI_OwnBook", "false"), ("BookFile", "no_book"), ("Threads", "2"), ("USI_Hash", "512")])
    with open(out_path, "a") as out:
        for gi, moves in enumerate(games):
            n = 0
            # Ply i (0-based) is the odds-holder's move when i is odd (White moves first).
            for i in range(2 * MIN_MOVE - 1, len(moves), 2):
                if n >= MAX_PER_GAME:
                    break
                pos = "sfen " + START + " moves " + " ".join(moves[:i])
                yane_mv = moves[i]
                our_mv, _ = ours.go(pos, "btime 0 wtime 0 byoyomi 1000")
                best, best_sc = judge.go(pos, "nodes 3000000")
                sc = {best: best_sc}
                for m in {yane_mv, our_mv} - {best}:
                    sc[m] = judge.go(pos, f"nodes 3000000 searchmoves {m}")[1]
                out.write(json.dumps({"game": f"{worker}:{gi}", "move_no": i // 2 + 1, "pos": pos,
                                      "yane": yane_mv, "ours": our_mv, "judge_best": best,
                                      "best": best_sc, "yane_sc": sc[yane_mv], "ours_sc": sc[our_mv]}) + "\n")
                out.flush()
                n += 1
    ours.send("quit"); judge.send("quit")


main()
