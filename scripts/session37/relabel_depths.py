# Label positions with YaneuraOu (Threads=1) at several depths, to see how the depth-8 training
# labels compare with deeper searches. A fresh engine per chunk; within a chunk each position is
# searched at the depths in increasing order (as iterative deepening would), scores from the side
# to move. Input: JSONL with an "sfen" field (board side hand), other fields are carried through.
# usage: python3 scripts/session37/relabel_depths.py IN.jsonl OUT.jsonl 8,12,16 WORKERS
import json, os, sys
from multiprocessing import Pool
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from label_games import Y  # noqa: E402

CHUNK = 200


def work(task):
    rows, depths = task
    y = Y(); out = []
    for r in rows:
        pos = "sfen " + r["sfen"] + " 1"
        for d in depths:
            _, sc, best = y.label(pos, d)
            r[f"d{d}"] = sc; r[f"best{d}"] = best
        out.append(r)
    y.send("quit")
    return out


if __name__ == "__main__":
    src, dst, depths, workers = sys.argv[1], sys.argv[2], [int(x) for x in sys.argv[3].split(",")], int(sys.argv[4])
    rows = [json.loads(l) for l in open(src)]
    tasks = [(rows[i:i + CHUNK], depths) for i in range(0, len(rows), CHUNK)]
    n = 0
    with Pool(workers, maxtasksperchild=1) as p, open(dst, "w") as f:
        for out in p.imap_unordered(work, tasks):
            for r in out:
                f.write(json.dumps(r) + "\n")
            n += len(out); print(f"{n}/{len(rows)}", flush=True)
