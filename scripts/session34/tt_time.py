# Fixed time per move over a game's positions (table kept): depth and nodes reached.
import subprocess,sys,time,threading,queue
import os as _os, sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))
from paths import REPO
eng,ms,step,movesfile=sys.argv[1],int(sys.argv[2]),int(sys.argv[3]),sys.argv[4]; opts=sys.argv[5:]
moves=open(movesfile).read().split()
START="1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
p=subprocess.Popen([eng],cwd=REPO,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,bufsize=1)
q=queue.Queue(); threading.Thread(target=lambda:[q.put(l) for l in p.stdout],daemon=True).start()
def send(c): p.stdin.write(c+"\n"); p.stdin.flush()
def until(t):
    out=[]
    while True:
        l=q.get(timeout=120); out.append(l)
        if l.startswith(t): return out
send("usi"); [send(f"setoption name {o.split('=')[0]} value {o.split('=')[1]}") for o in opts]; send("isready"); until("readyok"); send("usinewgame")
ds=[]; ns=[]
for k in range(0,len(moves),step):
    send("position sfen "+START+(" moves "+" ".join(moves[:k]) if k else "")); send(f"go btime 0 wtime 0 byoyomi {ms}")
    d=n=0
    for l in until("bestmove"):
        t=l.split()
        if t[:2]==["info","depth"]: d=int(t[2]); n=int(t[t.index("nodes")+1]) if "nodes" in t else n
    ds.append(d); ns.append(n); print(k,d,n,flush=True)
print(f"mean depth {sum(ds)/len(ds):.2f}  total nodes {sum(ns)/1e6:.0f}M")
send("quit")
