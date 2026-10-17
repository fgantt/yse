# Per-move analysis of a user game: ours (score, move) vs YaneuraOu (score, best, score of our move).
import subprocess,sys,threading,queue,json,os
import os as _os, sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))
from paths import REPO, require_yane
YANE = require_yane()
lines=[l for l in open(sys.argv[1]) if l.strip()]
start=[l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip()
moves=[l for l in lines if not l.startswith("#")][0].split()
ours_color=sys.argv[2]  # "b" or "w": which plies are ours
ynodes=int(sys.argv[3]) if len(sys.argv)>3 else 1000000
class E:
    def __init__(s,path,cwd,opts):
        s.p=subprocess.Popen([path],cwd=cwd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,bufsize=1,env={**os.environ,"SHOGI_PREFS_DIR":REPO+"/matches/s34_threads/prefs"})
        s.q=queue.Queue(); threading.Thread(target=lambda:[s.q.put(l) for l in s.p.stdout],daemon=True).start()
        s.send("usi"); [s.send(f"setoption name {k} value {v}") for k,v in opts]; s.send("isready"); s.until("readyok"); s.send("usinewgame")
    def send(s,c): s.p.stdin.write(c+"\n"); s.p.stdin.flush()
    def until(s,t):
        out=[]
        while True:
            l=s.q.get(timeout=300); out.append(l)
            if l.startswith(t): return out
    def go(s,pos,go):
        s.send("position "+pos); s.send("go "+go); out=s.until("bestmove"); sc=None
        for l in out:
            t=l.split()
            if t[:2]==["info","depth"] and "score" in t and "lowerbound" not in t and "upperbound" not in t:
                i=t.index("score"); sc=int(t[i+2]) if t[i+1]=="cp" else (30000 if not t[i+2].startswith("-") else -30000)
        return out[-1].split()[1],sc
ours=E(REPO+"/engines/usi-engine-stable",REPO,[("NNUEFile","nnue/default.bin"),("USI_Threads","4")])
yane=E(YANE,os.path.dirname(YANE),[("USI_OwnBook","false"),("BookFile","no_book"),("Threads","4"),("USI_Hash","512")])
first="w" if " w " in start else "b"
for i,mv in enumerate(moves):
    side=first if i%2==0 else ("b" if first=="w" else "w")
    if side!=ours_color: continue
    pos="sfen "+start.replace("sfen ","")+(" moves "+" ".join(moves[:i]) if i else "")
    om,os_=ours.go(pos,"btime 0 wtime 0 byoyomi 1000")
    ym,ys=yane.go(pos,f"nodes {ynodes}")
    ys_played=ys if mv==ym else yane.go(pos,f"nodes {ynodes} searchmoves {mv}")[1]
    r={"ply":i+1,"move_no":i//2+1,"played":mv,"ours_best":om,"ours":os_,"yane_best":ym,"yane":ys,"yane_played":ys_played,"loss":(ys or 0)-(ys_played or 0)}
    print(json.dumps(r),flush=True)
