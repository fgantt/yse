"""SPRT for usi_match results: the same test `usi_match --sprt-elo0/--sprt-elo1` runs live.

Usage (from the repo root):
    python3 scripts/sprt.py check a.csv [b.csv ...] [elo0 elo1]  # LLR of matches (pooled), running or not
    python3 scripts/sprt.py plan [elo0 elo1]                    # games needed, by true Elo (simulated)

Pentanomial GSPRT, normal approximation (OpenBench / Fishtest): opening pairs are the samples,
each scored 0, 1/4, 1/2, 3/4 or 1 for A. LLR = n (s1 - s0) (2 mean - s0 - s1) / (2 var), with
s = expected score at the bound's logistic Elo. Bounds ln(beta/(1-alpha)), ln((1-beta)/alpha);
alpha = beta = 0.05 gives [-2.94, +2.94]. Defaults elo0 = 0, elo1 = 40.
"""
import csv, math, random, sys

ALPHA = BETA = 0.05
LOWER, UPPER = math.log(BETA / (1 - ALPHA)), math.log((1 - BETA) / ALPHA)
MIN_PAIRS = 20  # as usi_match --sprt-min-pairs: a few pairs can stop the test on luck


def expected(elo):
    return 1 / (1 + 10 ** (-elo / 400))


def llr(pair_scores, elo0, elo1):
    n = len(pair_scores)
    if n < 2:
        return 0.0
    mean = sum(pair_scores) / n
    var = sum((x - mean) ** 2 for x in pair_scores) / n
    if var < 1e-9:
        return 0.0
    s0, s1 = expected(elo0), expected(elo1)
    return n * (s1 - s0) * (2 * mean - s0 - s1) / (2 * var)


def pair_scores_from_csv(path):
    rows = list(csv.DictReader(open(path)))
    pts = {"AWin": 1.0, "BWin": 0.0, "Draw": 0.5}
    games = [pts[r["result"]] for r in rows]
    return [(games[i] + games[i + 1]) / 2 for i in range(0, len(games) - 1, 2)]


def check(paths, elo0, elo1):
    scores = [x for p in paths for x in pair_scores_from_csv(p)]
    # Replay pair by pair: where would a live SPRT have stopped?
    for k in range(MIN_PAIRS, len(scores) + 1):
        v = llr(scores[:k], elo0, elo1)
        if v >= UPPER or v <= LOWER:
            print(f"live SPRT would have stopped after {2 * k} games: "
                  f"{'PASS' if v >= UPPER else 'FAIL'} (LLR {v:+.2f})")
            break
    v = llr(scores, elo0, elo1)
    state = "PASS" if v >= UPPER else "FAIL" if v <= LOWER else "continue"
    print(f"{' + '.join(paths)}: {2 * len(scores)} games, LLR {v:+.2f} [{LOWER:.2f}, {UPPER:.2f}] -> {state}")


def plan(elo0, elo1, draw_rate=0.02, max_games=1000, runs=2000):
    rng = random.Random(1)
    print(f"SPRT [{elo0:+}, {elo1:+}], alpha = beta = {ALPHA}, cap {max_games} games, "
          f"{draw_rate:.0%} draws")
    print("true Elo | pass rate | median games | 90th pct games")
    for true in (-40, -20, 0, 20, 40, 60, 80):
        p = expected(true)
        pw = p - draw_rate / 2
        lens, passes = [], 0
        for _ in range(runs):
            scores = []
            while True:
                pair = 0.0
                for _ in range(2):
                    r = rng.random()
                    pair += 1.0 if r < pw else 0.5 if r < pw + draw_rate else 0.0
                scores.append(pair / 2)
                v = llr(scores, elo0, elo1)
                if (len(scores) >= MIN_PAIRS and (v >= UPPER or v <= LOWER)) or 2 * len(scores) >= max_games:
                    break
            passes += v >= UPPER
            lens.append(2 * len(scores))
        lens.sort()
        print(f"{true:+8} | {passes / runs:9.0%} | {lens[len(lens) // 2]:12} | {lens[int(len(lens) * 0.9)]:14}")


if __name__ == "__main__":
    cmd = sys.argv[1] if len(sys.argv) > 1 else "plan"
    if cmd == "check":
        args = sys.argv[2:]
        paths = [a for a in args if a.endswith(".csv")]
        bounds = [float(a) for a in args if not a.endswith(".csv")]
        e0, e1 = bounds if len(bounds) == 2 else (0.0, 40.0)
        check(paths, e0, e1)
    else:
        e0, e1 = (float(sys.argv[2]), float(sys.argv[3])) if len(sys.argv) > 3 else (0.0, 40.0)
        plan(e0, e1)
