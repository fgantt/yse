# Re-judge compare.jsonl rows with uncapped YaneuraOu scores (mate = +-(100000 - plies)).
import json, os, subprocess, sys, threading, queue
YANE = "/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1"
inp, out_path, w, nw = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
rows = [json.loads(l) for l in open(inp)][w::nw]
p = subprocess.Popen([YANE], cwd=os.path.dirname(YANE), stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
q = queue.Queue(); threading.Thread(target=lambda: [q.put(l) for l in p.stdout], daemon=True).start()
def send(c): p.stdin.write(c + "\n"); p.stdin.flush()
def until(t):
    o = []
    while True:
        l = q.get(timeout=900); o.append(l)
        if l.startswith(t): return o
for c in ["usi", "setoption name USI_OwnBook value false", "setoption name BookFile value no_book", "setoption name Threads value 4", "setoption name USI_Hash value 512", "isready"]: send(c)
until("readyok")
def score(pos, extra=""):
    send("position " + pos); send("go nodes 3000000" + extra); o = until("bestmove"); s = None
    for l in o:
        t = l.split()
        if t[:2] == ["info", "depth"] and "score" in t and "lowerbound" not in t and "upperbound" not in t:
            i = t.index("score"); v = int(t[i + 2]) if t[i + 1] == "cp" else None
            if v is None:
                m = int(t[i + 2].replace("+", "")); v = (100000 - abs(m)) * (1 if m > 0 or t[i+2] == "+" else -1)
            s = v
    return o[-1].split()[1], s
with open(out_path, "a") as f:
    for r in rows:
        send("usinewgame")
        best, bs = score(r["pos"]); sc = {best: bs}
        for m in {r["yane"], r["ours"]} - {best}:
            sc[m] = score(r["pos"], " searchmoves " + m)[1]
        r.update(judge_best=best, best=bs, yane_sc=sc[r["yane"]], ours_sc=sc[r["ours"]])
        f.write(json.dumps(r) + "\n"); f.flush()
send("quit")
