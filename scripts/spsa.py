"""SPSA tuning of the lean search's constants (Session 40, RESEARCH_S40.md §4 C).

Usage (from the repo root):
    python3 scripts/spsa.py RUN_DIR --params PARAMS.json --engine ENGINE \
        [--iterations 20000] [--workers 12] [--byoyomi 100] [--threads 1]
    python3 scripts/spsa.py RUN_DIR --report

PARAMS.json: {"name": {"start": v, "min": lo, "max": hi, "c_end": c, "r_end": r}, ...}.
Names are those of src/search/lean_params.rs (passed in SHOGI_LEAN_PARAMS), plus
corr_pawn, corr_hand, corr_cont, corr_step (SHOGI_LEAN_CORR_W; SHOGI_LEAN_CORR=1 is set
whenever one of them is tuned, or with --corr).

Each iteration perturbs every parameter by +-c_k at random, plays one opening pair
(two games, colours swapped) between theta+ and theta-, and moves theta by
a_k * (wins - losses of theta+) * delta / c_k. Schedules are the usual ones
(gamma 0.101, alpha 0.602, A = N/10), set from each parameter's end-of-run
perturbation c_end and learning rate r_end = a_end / c_end^2, as in fishtest.
Workers update theta asynchronously; state is saved after every pair and a
rerun of the same command resumes.

The ideas are the standard SPSA method; no tuner code or tuned values of other
engines are used.
"""
import argparse, csv, json, os, random, subprocess, sys, tempfile, threading, time
from concurrent.futures import ThreadPoolExecutor

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
USI_MATCH = os.environ.get("USI_MATCH", os.path.join(REPO, "matches/s38/eng/usi_match"))
CORR = {"corr_pawn": 0, "corr_hand": 1, "corr_cont": 2, "corr_step": 3}
CORR_DEFAULT = [2, 2, 3, 7]
GAMMA, ALPHA = 0.101, 0.602


class Spsa:
    def __init__(self, run_dir, params, n, corr):
        self.dir, self.params, self.n, self.corr = run_dir, params, n, corr
        self.lock = threading.Lock()
        self.state_path = os.path.join(run_dir, "state.json")
        if os.path.exists(self.state_path):
            st = json.load(open(self.state_path))
            self.theta, self.k, self.score = st["theta"], st["k"], st["score"]
            self.issued = self.k  # pairs in flight at a stop are replayed
        else:
            self.theta = {p: float(v["start"]) for p, v in params.items()}
            self.k, self.issued, self.score = 0, 0, [0, 0, 0]
        A = 0.1 * n
        self.c = {p: v["c_end"] * n ** GAMMA for p, v in params.items()}
        self.a = {p: v["r_end"] * v["c_end"] ** 2 * (A + n) ** ALPHA for p, v in params.items()}
        self.A = A

    def next_pair(self):
        with self.lock:
            if self.issued >= self.n:
                return None
            self.issued += 1
            k = self.issued
            delta = {p: random.choice((-1, 1)) for p in self.params}
            ck = {p: self.c[p] / k ** GAMMA for p in self.params}
            plus, minus = {}, {}
            for p, v in self.params.items():
                t = self.theta[p]
                plus[p] = min(v["max"], max(v["min"], t + ck[p] * delta[p]))
                minus[p] = min(v["max"], max(v["min"], t - ck[p] * delta[p]))
            return k, delta, ck, plus, minus

    def update(self, k, delta, ck, result, wdl):
        with self.lock:
            for p, v in self.params.items():
                ak = self.a[p] / (self.A + k) ** ALPHA
                t = self.theta[p] + ak * result * delta[p] / ck[p]
                self.theta[p] = min(v["max"], max(v["min"], t))
            self.k += 1
            self.score = [s + x for s, x in zip(self.score, wdl)]
            tmp = self.state_path + ".tmp"
            json.dump({"theta": self.theta, "k": self.k, "score": self.score}, open(tmp, "w"), indent=1)
            os.replace(tmp, self.state_path)
            with open(os.path.join(self.dir, "theta.csv"), "a") as f:
                if self.k == 1 and f.tell() == 0:
                    f.write("k,time," + ",".join(self.params) + "\n")
                f.write(f"{self.k},{int(time.time())}," + ",".join(f"{self.theta[p]:.2f}" for p in self.params) + "\n")


def env_for(theta, corr):
    lean = ",".join(f"{p}={round(v)}" for p, v in theta.items() if p not in CORR)
    w = list(CORR_DEFAULT)
    for p, i in CORR.items():
        if p in theta:
            w[i] = round(theta[p])
    env = {"SHOGI_LEAN_PARAMS": lean}
    if corr or any(p in CORR for p in theta):
        env["SHOGI_LEAN_CORR"] = "1"
        env["SHOGI_LEAN_CORR_W"] = ",".join(map(str, w))
    else:
        env["SHOGI_LEAN_CORR"] = "0"  # on by default in the engine since Session 40
    return env


def wrapper(path, engine, env):
    with open(path, "w") as f:
        f.write("#!/bin/sh\n" + " ".join(f"{k}='{v}'" for k, v in env.items()) + f" exec '{engine}' \"$@\"\n")
    os.chmod(path, 0o755)


def play_pair(spsa, args, wid):
    work = os.path.join(spsa.dir, f"w{wid}")
    os.makedirs(os.path.join(work, "prefs"), exist_ok=True)
    while True:
        job = spsa.next_pair()
        if job is None:
            return
        k, delta, ck, plus, minus = job
        a, b = os.path.join(work, "plus"), os.path.join(work, "minus")
        wrapper(a, args.engine, env_for(plus, args.corr))
        wrapper(b, args.engine, env_for(minus, args.corr))
        out = os.path.join(work, "pair.csv")
        opts = (f"USI_Threads={args.threads};USI_OwnBook=false;USI_Hash={args.hash};"
                f"MoveOverhead={args.overhead}")
        cmd = [USI_MATCH, "--engine-a", a, "--cwd-a", REPO, "--options-a", opts,
               "--engine-b", b, "--cwd-b", REPO, "--options-b", opts,
               "--byoyomi", str(args.byoyomi), "--games", "2",
               "--seed", str(random.randrange(1 << 31)), "--output", out]
        env = dict(os.environ, SHOGI_PREFS_DIR=os.path.join(work, "prefs"))
        r = subprocess.run(cmd, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        if r.returncode != 0 or not os.path.exists(out):
            print(f"pair {k}: usi_match failed ({r.returncode}); skipped", flush=True)
            with spsa.lock:
                spsa.issued -= 1
            time.sleep(5)
            continue
        wdl = [0, 0, 0]
        for row in csv.DictReader(open(out)):
            wdl[{"AWin": 0, "Draw": 1, "BWin": 2}[row["result"]]] += 1
        spsa.update(k, delta, ck, wdl[0] - wdl[2], wdl)
        if spsa.k % 50 == 0:
            w, d, l = spsa.score
            print(f"{time.strftime('%H:%M')} pairs {spsa.k}/{spsa.n}  theta+ W{w} D{d} L{l}  "
                  + " ".join(f"{p}={v:.1f}" for p, v in spsa.theta.items()), flush=True)


def report(run_dir):
    st = json.load(open(os.path.join(run_dir, "state.json")))
    cfg = json.load(open(os.path.join(run_dir, "config.json")))
    print(f"pairs {st['k']}/{cfg['iterations']}  W/D/L of theta+ {st['score']}")
    for p, v in cfg["params"].items():
        print(f"  {p:18} start {v['start']:>6}  now {st['theta'][p]:8.2f}  range [{v['min']}, {v['max']}]")
    print("SHOGI_LEAN_PARAMS=" + env_for(st["theta"], False)["SHOGI_LEAN_PARAMS"])


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("run_dir")
    ap.add_argument("--params")
    ap.add_argument("--engine")
    ap.add_argument("--iterations", type=int, default=20000)
    ap.add_argument("--workers", type=int, default=12)
    ap.add_argument("--byoyomi", type=int, default=100)
    ap.add_argument("--threads", type=int, default=1)
    ap.add_argument("--hash", type=int, default=64)
    ap.add_argument("--overhead", type=int, default=20,
                    help="MoveOverhead ms; the default 100 leaves 100 ms byoyomi a 10 ms search")
    ap.add_argument("--corr", action="store_true", help="correction history on in both engines")
    ap.add_argument("--report", action="store_true")
    args = ap.parse_args()
    if args.report:
        return report(args.run_dir)
    os.makedirs(args.run_dir, exist_ok=True)
    cfg_path = os.path.join(args.run_dir, "config.json")
    if os.path.exists(cfg_path):
        cfg = json.load(open(cfg_path))
        params, args.iterations = cfg["params"], cfg["iterations"]
        args.engine, args.byoyomi, args.corr = cfg["engine"], cfg["byoyomi"], cfg["corr"]
        args.threads, args.hash, args.overhead = cfg["threads"], cfg["hash"], cfg["overhead"]
    else:
        params = json.load(open(args.params))
        json.dump({"params": params, "iterations": args.iterations, "engine": os.path.abspath(args.engine),
                   "byoyomi": args.byoyomi, "corr": args.corr, "threads": args.threads, "hash": args.hash,
                   "overhead": args.overhead},
                  open(cfg_path, "w"), indent=1)
        args.engine = os.path.abspath(args.engine)
    spsa = Spsa(args.run_dir, params, args.iterations, args.corr)
    with ThreadPoolExecutor(args.workers) as ex:
        for f in [ex.submit(play_pair, spsa, args, i) for i in range(args.workers)]:
            f.result()
    report(args.run_dir)


if __name__ == "__main__":
    main()
