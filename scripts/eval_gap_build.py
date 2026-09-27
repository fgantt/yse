"""Build the eval-divergence position set from user games against YaneuraOu.

Every position of every game in `matches/user_games/` (both sides to move) is
labelled by YaneuraOu (fixed nodes, one thread) and tagged with features read
from the board alone -- so subsets such as "our king under attack" do not
depend on what our engine thinks.

Game files: one line per move pair, `N <black move> <white move>`; in a
handicap game White moves first, so line 1 holds only White's move. The start
position comes from the file name: `6mai` or `8mai`.

Output: JSONL, one position per line, scores from Black's (our) point of view.

    python3 scripts/eval_gap_build.py [--nodes 3000000] [--out eval_gap/positions.jsonl]
"""
import argparse, glob, json, os, subprocess

YANE = ("/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/"
        "NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1")
STARTS = {
    "6mai": "2sgkgs2/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
    "8mai": "3gkg3/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1",
}
CLAMP = 3000


class Yane:
    def __init__(self):
        self.p = subprocess.Popen([YANE], cwd=os.path.dirname(YANE), stdin=subprocess.PIPE,
                                  stdout=subprocess.PIPE, text=True, bufsize=1)
        self.send("usi"); self.until("usiok")
        for o in ["USI_OwnBook value false", "BookFile value no_book", "Threads value 1",
                  "USI_Hash value 256"]:
            self.send("setoption name " + o)
        self.send("isready"); self.until("readyok")

    def send(self, s):
        self.p.stdin.write(s + "\n"); self.p.stdin.flush()

    def until(self, tok):
        out = []
        while True:
            line = self.p.stdout.readline().strip(); out.append(line)
            if line.startswith(tok):
                return out

    def sfen(self, pos):
        self.send(pos); self.send("d"); self.send("isready")
        return next(l for l in self.until("readyok") if l.startswith("sfen "))[5:]

    def search(self, pos, nodes):
        self.send("usinewgame"); self.send(pos); self.send(f"go nodes {nodes}")
        out = self.until("bestmove")
        cp, mate = None, None
        for l in out:
            t = l.split()
            if t[:1] == ["info"] and "score" in t:
                i = t.index("score")
                v = int(t[i + 2])
                cp, mate = (v, None) if t[i + 1] == "cp" else (None, v)
        if mate is not None:
            cp = CLAMP if mate > 0 or str(mate).startswith("+") else -CLAMP
        return out[-1].split()[1], max(-CLAMP, min(CLAMP, cp)), mate


def parse_sfen(sfen):
    """Board as {(row, col): (piece, is_black)}, rows 0..8 = a..i, cols 0..8 = files 9..1."""
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
                c += 1; promo = False
    hands = {True: {}, False: {}}
    if hand != "-":
        n = ""
        for ch in hand:
            if ch.isdigit():
                n += ch
            else:
                hands[ch.isupper()][ch.upper()] = int(n or 1); n = ""
    return board, side, hands


def king_features(board, hands, black):
    """Pressure on `black`'s king (True = Black, ours): enemy pieces within two
    squares, enemy pieces in hand other than pawns, own defenders adjacent."""
    king = next((sq for sq, (p, b) in board.items() if p == "K" and b == black), None)
    if king is None:
        return {}
    near = lambda sq, d: max(abs(sq[0] - king[0]), abs(sq[1] - king[1])) <= d
    enemy = [p for sq, (p, b) in board.items() if b != black and near(sq, 2)]
    return {
        "enemy_near": len(enemy),
        "enemy_promoted_near": sum(p.startswith("+") for p in enemy),
        "enemy_hand": sum(n for p, n in hands[not black].items() if p != "P"),
        "defenders_adjacent": sum(1 for sq, (p, b) in board.items()
                                  if b == black and p != "K" and near(sq, 1)),
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--nodes", type=int, default=3_000_000)
    ap.add_argument("--games", default="matches/user_games/*.txt")
    ap.add_argument("--out", default="eval_gap/positions.jsonl")
    a = ap.parse_args()
    os.makedirs(os.path.dirname(a.out), exist_ok=True)
    y = Yane()
    with open(a.out, "w") as f:
        for path in sorted(glob.glob(a.games)):
            name = os.path.basename(path)[:-4]
            start = next(s for k, s in STARTS.items() if k in name)
            moves = [m for line in open(path) for m in line.split()[1:]]
            for k in range(len(moves) + 1):
                pos = f"position sfen {start} moves " + " ".join(moves[:k])
                sfen = y.sfen(pos)
                board, side, hands = parse_sfen(sfen)
                best, cp, mate = y.search(pos, a.nodes)
                black_to_move = side == "b"
                row = {
                    "game": name, "ply": k, "stm": side, "sfen": sfen,
                    "yo_cp": cp if black_to_move else -cp,  # Black's point of view
                    "yo_mate": mate, "yo_best": best,
                    "played": moves[k] if k < len(moves) else None,
                    "our_king": king_features(board, hands, True),
                    "their_king": king_features(board, hands, False),
                }
                f.write(json.dumps(row) + "\n"); f.flush()
    y.send("quit")


if __name__ == "__main__":
    main()
