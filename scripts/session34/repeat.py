# Repeat one position N times in one engine; flag any search that does not answer.
import subprocess,sys,time,threading,queue
import os as _os, sys as _sys
_sys.path.insert(0, _os.path.dirname(_os.path.dirname(_os.path.abspath(__file__))))
from paths import REPO
eng,sfen,go,reps=sys.argv[1],open(sys.argv[2]).read().strip(),sys.argv[3],int(sys.argv[4])
p=subprocess.Popen([eng],cwd=REPO,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,bufsize=1)
Q=queue.Queue()
threading.Thread(target=lambda: [Q.put(l) for l in p.stdout],daemon=True).start()
def send(s): p.stdin.write(s+"\n"); p.stdin.flush()
def until(tok,cap):
    t=time.time()
    while time.time()-t<cap:
        try: l=Q.get(timeout=0.5)
        except queue.Empty: continue
        if l.startswith(tok): return time.time()-t
    return None
send("usi"); [send(f"setoption name {o.split('=')[0]} value {o.split('=')[1]}") for o in sys.argv[5:]]; send("isready"); until("readyok",30)
hangs=0; times=[]
for r in range(reps):
    send("position sfen "+sfen); send("go "+go); dt=until("bestmove",15)
    if dt is None:
        hangs+=1; send("stop"); print("HANG rep",r,"; after stop:",until("bestmove",10),flush=True); dt=15
    times.append(dt)
print(f"reps {reps} hangs {hangs} median {sorted(times)[reps//2]:.2f}s max {max(times):.2f}s")
send("quit")
