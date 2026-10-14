# Classify our moves and YaneuraOu's on the odds side's positions (from odds_compare.py).
#   python3 odds_classify.py compare.jsonl
import json, sys, collections

PROMOTED = {"+P": "tokin", "+L": "+L", "+N": "+N", "+S": "+S", "+B": "horse", "+R": "dragon"}
NAMES = {"P": "pawn", "L": "lance", "N": "knight", "S": "silver", "G": "gold", "B": "bishop",
         "R": "rook", "K": "king"}


def parse_sfen(sfen):
    board = {}
    rows = sfen.split()[0].split("/")
    for r, row in enumerate(rows):  # r 0 = rank a
        f = 9
        i = 0
        while i < len(row):
            c = row[i]
            if c.isdigit():
                f -= int(c); i += 1; continue
            p = c
            if c == "+":
                p = "+" + row[i + 1]; i += 1
            board[(f, r)] = p
            f -= 1; i += 1
    return board


def sq(s):
    return int(s[0]), ord(s[1]) - ord("a")


def apply(board, mv):
    if "*" in mv:
        pt, to = mv.split("*")
        board[sq(to)] = pt if board_side_upper(mv, board) else pt.lower()
        return
    fr, to, promo = sq(mv[0:2]), sq(mv[2:4]), mv.endswith("+")
    p = board.pop(fr)
    if promo:
        p = "+" + p
    board[to] = p


def board_side_upper(mv, board):
    return APPLY_SIDE[0]


APPLY_SIDE = [True]


def replay(pos):
    parts = pos.split(" moves ")
    board = parse_sfen(parts[0].replace("sfen ", ""))
    moves = parts[1].split() if len(parts) > 1 else []
    side_upper = " b " in parts[0]  # side to move at the start
    for mv in moves:
        APPLY_SIDE[0] = side_upper
        apply(board, mv)
        side_upper = not side_upper
    return board, side_upper


def classify(board, mv, upper):
    """Tags for `mv` by the side to move (`upper`: Black, pieces in upper case)."""
    tags = []
    enemy_camp = (lambda r: r <= 2) if upper else (lambda r: r >= 6)
    if "*" in mv:
        pt, to = mv.split("*")
        f, r = sq(to)
        tags.append("drop")
        tags.append(f"drop:{NAMES[pt]}")
        # The opponent's most advanced pawn on that file.
        pawns = [rr for (ff, rr), p in board.items() if ff == f and p == ("p" if upper else "P")]
        if enemy_camp(r):
            tags.append("drop in enemy camp")
        if pawns:
            front = max(pawns) if upper else min(pawns)
            if (r < front) if upper else (r > front):
                tags.append("drop behind enemy pawn")
        return tags
    fr, to = sq(mv[0:2]), sq(mv[2:4])
    p = board.get(fr, "?")
    key = p.upper() if not p.startswith("+") else "+" + p[1].upper()
    name = PROMOTED.get(key, NAMES.get(key, key))
    tags.append(f"move:{name}")
    target = board.get(to)
    if target is not None:
        tags.append("capture")
    if mv.endswith("+"):
        tags.append("promotes")
    if to in [(1, 0), (9, 0), (1, 8), (9, 8)]:
        tags.append("to corner")
    if enemy_camp(to[1]):
        tags.append("into enemy camp")
    return tags


def main():
    rows = [json.loads(l) for l in open(sys.argv[1])]
    n = len(rows)
    agree = sum(r["yane"] == r["ours"] for r in rows)
    tag_y, tag_o = collections.Counter(), collections.Counter()
    losses = []
    dis = []
    for r in rows:
        board, upper = replay(r["pos"])
        ty, to_ = classify(board, r["yane"], upper), classify(board, r["ours"], upper)
        tag_y.update(set(ty)); tag_o.update(set(to_))
        loss = r["yane_sc"] - r["ours_sc"]  # judge: what our move costs against Yane's
        losses.append(loss)
        if r["yane"] != r["ours"]:
            dis.append((loss, r, ty, to_))
    print(f"{n} positions; same move as YaneuraOu {agree} ({100*agree/n:.0f}%)")
    print(f"our move vs YaneuraOu (judge 3M nodes, uncapped): mean {sum(min(l,3000) for l in losses)/n:+.0f} cp (each capped at 3000), median {sorted(losses)[n//2]}, "
          f">=150 cp worse on {sum(l >= 150 for l in losses)}, >=400 on {sum(l >= 400 for l in losses)}")
    print("\nmove kind                 YaneuraOu   ours")
    for t in sorted(set(tag_y) | set(tag_o), key=lambda t: -(tag_y[t] + tag_o[t])):
        print(f"  {t:24s} {tag_y[t]:6d}   {tag_o[t]:6d}")
    big = [d for d in dis if d[0] >= 150]
    print(f"\n{len(big)} disagreements costing >=150 cp: what YaneuraOu played vs what we played")
    pair = collections.Counter()
    for loss, r, ty, to_ in big:
        a = next((t for t in ty if t.startswith(("drop:", "move:"))), "?")
        b = next((t for t in to_ if t.startswith(("drop:", "move:"))), "?")
        pair[(a, b)] += 1
    for (a, b), c in pair.most_common(15):
        print(f"  {c:3d}  Yane {a:16s} | ours {b}")
    yb = collections.Counter(t for _, _, ty, _ in big for t in set(ty))
    ob = collections.Counter(t for _, _, _, to_ in big for t in set(to_))
    print("\n  in those, tag counts      YaneuraOu   ours")
    for t in sorted(set(yb) | set(ob), key=lambda t: -(yb[t] + ob[t]))[:14]:
        print(f"  {t:24s} {yb[t]:6d}   {ob[t]:6d}")
    print("\nworst 12:")
    for loss, r, ty, to_ in sorted(big, key=lambda d: -d[0])[:12]:
        print(f"  {loss:5d} cp  game {r['game']} move {r['move_no']}: Yane {r['yane']} {ty} | ours {r['ours']} {to_}")


main()
