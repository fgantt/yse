"""Neutral-set move loss: engines pick moves at a time control, YaneuraOu judges.

Usage (from the repo root):
    python3 scripts/move_loss.py <positions> <judge depth> name=engine [name=engine]
    e.g. python3 scripts/move_loss.py 150 10 old=engines/usi-engine-old new=target/release/usi-engine

Positions: evenly spaced from the held-out teacher self-play games
(`nnue_corpus_yaneura_d10.jsonl`, `game_id % 20 == 0`, excluded from training
since Session 29). Engines run from the repo root, so each loads
`nnue/default.bin`, with default options, at `byoyomi 1000`, one at a time.
With two engines it also reports the paired difference. Per-position rows go
to `move_loss_rows.jsonl` in the current directory.

Not comparable with the Session 28-29 move-loss figures, whose tooling was
lost: this scores the judge's own best move as 0 and the played move with
`searchmoves` from the same search, so its medians sit near 0.

loss = clamp(best score) - clamp(score of the played move), both from
`go depth D` on the same position (the second with `searchmoves`), 0 when the
engine played the judge's best move. Scores clamp to +-3000 cp; mate = +-3000.
"""
import json, subprocess, sys, statistics

CORPUS = "nnue_corpus_yaneura_d10.jsonl"
YANE = "/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1"
CAP = 3000

def neutral_positions(n):
    held = [json.loads(l)["sfen"] for l in open(CORPUS) if json.loads(l)["game_id"] % 20 == 0]
    step = len(held) / n
    return [held[int(i * step)] for i in range(n)]

class Engine:
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
    def go(self, sfen, go):
        self.send("position sfen " + sfen); self.send("go " + go)
        lines = self.until("bestmove")
        return lines[-1].split()[1], lines
    def quit(self): self.send("quit")

def score_of(lines):
    s = None
    for l in lines:
        t = l.split()
        if t[:1] == ["info"] and "score" in t and "lowerbound" not in t and "upperbound" not in t:
            i = t.index("score")
            if t[i + 1] == "cp": s = int(t[i + 2])
            elif t[i + 1] == "mate":
                m = t[i + 2]; s = -CAP if m.startswith("-") else CAP
    return max(-CAP, min(CAP, s if s is not None else 0))

def main():
    n, depth = int(sys.argv[1]), int(sys.argv[2])
    engines = sys.argv[3:]  # name=path pairs
    sfens = neutral_positions(n)
    played = {}
    for spec in engines:
        name, path = spec.split("=", 1)
        e = Engine(path)
        moves = []
        for sf in sfens:
            mv, _ = e.go(sf, "btime 0 wtime 0 byoyomi 1000")
            moves.append(mv)
        e.quit(); played[name] = moves
        print(f"{name}: {n} moves chosen", flush=True)
    judge = Engine(YANE, cwd=YANE.rsplit("/", 1)[0],
                   options=["setoption name USI_OwnBook value false",
                            "setoption name BookFile value no_book",
                            "setoption name Threads value 1",
                            "setoption name USI_Hash value 256"])
    losses = {k: [] for k in played}
    rows = []
    for i, sf in enumerate(sfens):
        best, lines = judge.go(sf, f"depth {depth}")
        best_score = score_of(lines)
        row = {"i": i, "sfen": sf, "judge_best": best, "judge_score": best_score}
        cache = {best: best_score}
        for k, moves in played.items():
            mv = moves[i]
            if mv not in cache:
                if mv in ("resign", "win", "none"):
                    cache[mv] = -CAP
                else:
                    _, l2 = judge.go(sf, f"depth {depth} searchmoves {mv}")
                    cache[mv] = score_of(l2)
            loss = max(0, best_score - cache[mv])
            losses[k].append(loss); row[k] = [mv, loss]
        rows.append(row)
    judge.quit()
    with open("move_loss_rows.jsonl", "w") as f:
        for r in rows: f.write(json.dumps(r) + "\n")
    for k, L in losses.items():
        print(f"{k}: mean {statistics.mean(L):.0f}  median {statistics.median(L):.0f}  "
              f">200cp {100*sum(x>200 for x in L)/len(L):.0f}%  best-move {100*sum(x==0 for x in L)/len(L):.0f}%")
    if len(played) == 2:
        a, b = list(losses)
        d = [x - y for x, y in zip(losses[b], losses[a])]
        sd = statistics.stdev(d)
        print(f"paired {b}-{a}: mean {statistics.mean(d):+.1f} cp (SE {sd/len(d)**0.5:.1f}), "
              f"{b} better on {sum(x<0 for x in d)}, worse on {sum(x>0 for x in d)}, same {sum(x==0 for x in d)}")

main()
