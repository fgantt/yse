"""King-threat features from an SFEN, for the feasibility test of threat inputs.

Board: rows 0..8 = ranks a..i, cols 0..8 = files 9..1. Black moves towards row 0.
"""

STEP = {  # (drow, dcol) for Black; White mirrors drow
    "P": [(-1, 0)],
    "N": [(-2, -1), (-2, 1)],
    "S": [(-1, -1), (-1, 0), (-1, 1), (1, -1), (1, 1)],
    "G": [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, 0)],
    "K": [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)],
}
for p in ("+P", "+L", "+N", "+S"):
    STEP[p] = STEP["G"]
SLIDE = {
    "L": [(-1, 0)],
    "B": [(-1, -1), (-1, 1), (1, -1), (1, 1)],
    "R": [(-1, 0), (1, 0), (0, -1), (0, 1)],
}
SLIDE["+B"] = SLIDE["B"]
SLIDE["+R"] = SLIDE["R"]
STEP["+B"] = [(-1, 0), (1, 0), (0, -1), (0, 1)]
STEP["+R"] = [(-1, -1), (-1, 1), (1, -1), (1, 1)]
HAND_TYPES = ["P", "L", "N", "S", "G", "B", "R"]


def parse(sfen):
    board_s, side, hand = sfen.split()[:3]
    board = {}
    for r, rank in enumerate(board_s.split("/")):
        c, promo = 0, False
        for ch in rank:
            if ch.isdigit():
                c += int(ch)
            elif ch == "+":
                promo = True
            else:
                board[(r, c)] = (("+" if promo else "") + ch.upper(), ch.isupper())
                c += 1
                promo = False
    hands = {True: dict.fromkeys(HAND_TYPES, 0), False: dict.fromkeys(HAND_TYPES, 0)}
    if hand != "-":
        n = ""
        for ch in hand:
            if ch.isdigit():
                n += ch
            else:
                hands[ch.isupper()][ch.upper()] += int(n or 1)
                n = ""
    return board, side == "b", hands


def attacks(board, black):
    """Count of `black`'s attacks on each square: {square: n}."""
    out = {}
    sgn = 1 if black else -1
    for (r, c), (p, b) in board.items():
        if b != black:
            continue
        for dr, dc in STEP.get(p, []):
            sq = (r + dr * sgn, c + dc)
            if 0 <= sq[0] < 9 and 0 <= sq[1] < 9:
                out[sq] = out.get(sq, 0) + 1
        for dr, dc in SLIDE.get(p, []):
            rr, cc = r + dr * sgn, c + dc
            while 0 <= rr < 9 and 0 <= cc < 9:
                out[(rr, cc)] = out.get((rr, cc), 0) + 1
                if (rr, cc) in board:
                    break
                rr += dr * sgn
                cc += dc
    return out


def king_features(board, hands, black, att_enemy, att_own):
    """Threat features on `black`'s king (True = Black)."""
    king = next((sq for sq, (p, b) in board.items() if p == "K" and b == black), None)
    if king is None:
        return None
    zone = [(king[0] + dr, king[1] + dc) for dr in (-1, 0, 1) for dc in (-1, 0, 1)
            if 0 <= king[0] + dr < 9 and 0 <= king[1] + dc < 9]
    neigh = [sq for sq in zone if sq != king]
    enemy_hand = hands[not black]
    att_pairs = sum(att_enemy.get(sq, 0) for sq in zone)
    att_squares = sum(1 for sq in zone if att_enemy.get(sq, 0))
    def_pairs = sum(att_own.get(sq, 0) for sq in neigh)
    undefended_att = sum(1 for sq in neigh if att_enemy.get(sq, 0) > att_own.get(sq, 0))
    escapes = sum(1 for sq in neigh
                  if not att_enemy.get(sq, 0) and not (sq in board and board[sq][1] == black))
    empty_neigh = sum(1 for sq in neigh if sq not in board)
    near2 = [p for sq, (p, b) in board.items()
             if b != black and max(abs(sq[0] - king[0]), abs(sq[1] - king[1])) <= 2]
    defenders = sum(1 for sq in neigh if sq in board and board[sq][1] == black)
    in_check = 1 if att_enemy.get(king, 0) else 0
    hand_major = enemy_hand["R"] + enemy_hand["B"]
    hand_gs = enemy_hand["G"] + enemy_hand["S"]
    hand_minor = enemy_hand["N"] + enemy_hand["L"]
    hand_total_np = hand_major + hand_gs + hand_minor
    # Drop squares: empty neighbours the enemy already attacks (a dropped gold or
    # silver there would be supported).
    supported_drop_sq = sum(1 for sq in neigh if sq not in board and att_enemy.get(sq, 0))
    king_rank_from_home = (8 - king[0]) if black else king[0]
    return [
        att_pairs, att_squares, def_pairs, undefended_att, escapes, empty_neigh,
        len(near2), sum(1 for p in near2 if p.startswith("+") or p in ("R", "B")),
        defenders, in_check, hand_major, hand_gs, hand_minor, enemy_hand["P"],
        supported_drop_sq, supported_drop_sq * hand_gs, att_pairs * hand_total_np,
        undefended_att * (1 + hand_total_np), king_rank_from_home,
    ]


NAMES = ["att_pairs", "att_squares", "def_pairs", "undefended_att", "escapes", "empty_neigh",
         "enemy_near2", "enemy_strong_near2", "defenders", "in_check", "hand_major", "hand_gs",
         "hand_minor", "hand_pawn", "supported_drop_sq", "drop_sq_x_gs", "att_x_hand",
         "undef_x_hand", "king_rank"]


def features(sfen):
    """Feature vector (Black-POV interactions) or None."""
    board, black_to_move, hands = parse(sfen)
    ab, aw = attacks(board, True), attacks(board, False)
    fb = king_features(board, hands, True, aw, ab)   # danger to Black's king
    fw = king_features(board, hands, False, ab, aw)  # danger to White's king
    if fb is None or fw is None:
        return None
    stm = 1.0 if black_to_move else -1.0
    # Danger to our king counts against Black, to theirs for Black; each again
    # times side to move (whose attack is "live").
    x = [1.0, stm]
    x += [-v for v in fb] + [v for v in fw]
    x += [-v * stm for v in fb] + [v * stm for v in fw]
    return x
