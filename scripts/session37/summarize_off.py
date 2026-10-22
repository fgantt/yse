# Summarize probe_off.py output against YaneuraOu's 2M-node verdicts (Threads=1, halfkpvm).
# usage: python3 scripts/session37/summarize_off.py matches/s37/off/*.jsonl
import json, sys
REF = {("2026-10-21_2mai_loss119_windows.txt", 41): (143, "8a7b"),
       ("2026-10-21_2mai_loss119_windows.txt", 42): (-296, "6i7i"),
       ("2026-10-21_2mai_loss119_windows.txt", 43): (-447, "6i7i"),
       ("2026-10-21_2mai_loss119_windows.txt", 44): (-597, "7i7h"),
       ("2026-10-21_2mai_loss119_windows.txt", 45): (-1532, "7i7h"),
       ("2026-10-21_2mai_loss127_windows_q9.txt", 45): (286, "S*5d"),
       ("2026-10-21_2mai_loss127_windows_q9.txt", 46): (-175, "5h4g"),
       ("2026-10-21_2mai_loss127_windows_q9.txt", 47): (-493, "4h4g"),
       ("2026-10-21_2mai_loss127_windows_q9.txt", 48): (-626, "N*6d"),
       ("2026-10-21_2mai_loss127_windows_q9.txt", 49): (-1653, "P*4h")}
rows = {}
for f in sys.argv[1:]:
    for l in open(f):
        r = json.loads(l); rows.setdefault((r["off"], r["budget"]), []).append(r)
print(f"{'off':<12} {'budget':>6} {'n':>3} {'gap mean':>9} {'gap med':>8} {'yane move':>9} {'depth':>6} {'secs':>6}   per-position score")
for (off, b), rs in sorted(rows.items(), key=lambda x: (x[0][1], x[0][0] != "-", x[0][0])):
    gaps = sorted(r["score"] - REF[(r["game"], r["k"])][0] for r in rs if "score" in r)
    hit = sum(r["best"] == REF[(r["game"], r["k"])][1] for r in rs)
    print(f"{off:<12} {b/1e6:>5.0f}M {len(rs):>3} {sum(gaps)/len(gaps):>9.0f} {gaps[len(gaps)//2]:>8} {hit:>6}/{len(rs):<2} "
          f"{sum(r.get('depth',0) for r in rs)/len(rs):>6.1f} {sum(r['secs'] for r in rs):>6.0f}   "
          + " ".join(f"{r.get('score','?'):>5}" for r in rs))
