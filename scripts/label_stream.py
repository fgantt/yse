"""Label large usi_match game sets with YaneuraOu, resumably (Session 35).

`label_games.py` keeps each worker's share in memory and writes at the end; at tens of
millions of positions one interruption loses hours. This labels batches of games, writes
each batch to its own file (atomically), and skips batches already written, so it can be
stopped and restarted with a different worker count.
    python3 scripts/label_stream.py OUT_DIR DEPTH WORKERS BOARD csv [csv ...]
Output: OUT_DIR/b_NNNNN.jsonl in the trainer format of `label_games.py`. Positions within
the opening's random plies (`--random-plies`, read from RANDOM_PLIES, default 8) are kept
and marked `from_random_opening`.
"""
import csv, json, os, sys
from multiprocessing import Pool

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from label_games import Y  # noqa: E402

BATCH = 500
RANDOM_PLIES = int(os.environ.get("RANDOM_PLIES", "8"))
_y = None


def init():
    global _y
    _y = Y()


def work(task):
    path, depth, games = task
    rows = []
    for gid, moves, outcome, start in games:
        for ply in range(len(moves) + 1):
            pos = "sfen " + start + (" moves " + " ".join(moves[:ply]) if ply else "")
            sfen, sc, best = _y.label(pos, depth)
            if sc is None or best in ("resign", "win"):
                break
            stm = sfen.split()[1]
            rows.append({"game_id": gid, "ply": ply + 1, "sfen": " ".join(sfen.split()[:3]),
                         "player": "black" if stm == "b" else "white", "eval_cp": sc,
                         "outcome": outcome,
                         "outcome_str": {1: "black_win", -1: "white_win", 0: "draw"}[outcome],
                         "from_random_opening": ply < RANDOM_PLIES, "random_move": False})
        _y.send("usinewgame")
    tmp = path + ".tmp"
    with open(tmp, "w") as f:
        for r in rows:
            f.write(json.dumps(r, separators=(",", ":")) + "\n")
    os.replace(tmp, path)
    return path, len(rows)


if __name__ == "__main__":
    out_dir, depth, workers, board = sys.argv[1], int(sys.argv[2]), int(sys.argv[3]), sys.argv[4]
    os.makedirs(out_dir, exist_ok=True)
    games = []
    gid = 1_000_000
    for fname in sys.argv[5:]:
        for r in csv.DictReader(open(fname)):
            moves = r["record"].split()
            res, a_black = r["result"], r["a_color"] == "black"
            outcome = 0 if res not in ("AWin", "BWin") else (1 if (res == "AWin") == a_black else -1)
            if moves:
                stm = "w" if moves[0][1] in "abc" else "b"
                games.append((gid, moves, outcome, f"{board} {stm} - 1"))
            gid += 1
    tasks = []
    for k in range(0, len(games), BATCH):
        path = os.path.join(out_dir, f"b_{k // BATCH:05d}.jsonl")
        if not os.path.exists(path):
            tasks.append((path, depth, games[k:k + BATCH]))
    total = (len(games) + BATCH - 1) // BATCH
    print(f"{len(games)} games, {total} batches, {total - len(tasks)} already done", flush=True)
    n = 0
    with Pool(workers, initializer=init) as p:
        for i, (path, rows) in enumerate(p.imap_unordered(work, tasks), 1):
            n += rows
            print(f"batch {os.path.basename(path)} {rows} positions; {i}/{len(tasks)} this run, "
                  f"{n} positions", flush=True)
