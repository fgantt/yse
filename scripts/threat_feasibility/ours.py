"""Score positions with our engine (go depth 1, single-threaded): stdin JSONL with sfen -> stdout JSONL + ours (side-to-move POV)."""
import json, subprocess, sys
from concurrent.futures import ThreadPoolExecutor as ProcessPoolExecutor
ENGINE, CWD, JOBS = sys.argv[1], sys.argv[2], int(sys.argv[3])
def run(rows):
    e = subprocess.Popen([ENGINE], cwd=CWD, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, bufsize=1)
    send = lambda s: (e.stdin.write(s + "\n"), e.stdin.flush())
    def until(t):
        o = []
        while True:
            l = e.stdout.readline()
            if not l: raise RuntimeError("engine died")
            o.append(l.strip())
            if l.startswith(t): return o
    send("usi"); until("usiok"); send("setoption name ParallelEnable value false"); send("isready"); until("readyok")
    out = []
    for r in rows:
        send("usinewgame"); send("position sfen " + r["sfen"]); send("go depth 1"); o = until("bestmove")
        cp = None
        for l in o:
            t = l.split()
            if t[:1] == ["info"] and "score" in t:
                i = t.index("score"); v = int(t[i + 2]); cp = v if t[i + 1] == "cp" else (3000 if v > 0 else -3000)
        r["ours"] = max(-3000, min(3000, cp if cp is not None else -3000)); out.append(r)
    send("quit"); return out
rows = [json.loads(l) for l in sys.stdin]
with ProcessPoolExecutor(JOBS) as ex:
    for part in ex.map(run, [rows[i::JOBS] for i in range(JOBS)]):
        for r in part: print(json.dumps(r))
