# Time to a fixed depth over a game's positions, one engine, table kept between moves.
import subprocess,sys,time,threading,queue,os
REPO="/Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue"
eng,depth,step,movesfile=sys.argv[1],int(sys.argv[2]),int(sys.argv[3]),sys.argv[4]; opts=sys.argv[5:]
moves=open(movesfile).read().split()
START="1nsgkgsn1/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"
p=subprocess.Popen([eng],cwd=REPO,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,bufsize=1)
q=queue.Queue(); threading.Thread(target=lambda:[q.put(l) for l in p.stdout],daemon=True).start()
def send(c): p.stdin.write(c+"\n"); p.stdin.flush()
def until(t):
    while True:
        l=q.get(timeout=900)
        if l.startswith(t): return
send("usi"); [send(f"setoption name {o.split('=')[0]} value {o.split('=')[1]}") for o in opts]; send("isready"); until("readyok"); send("usinewgame")
tot=0; ts=[]
for k in range(0,len(moves),step):
    send("position sfen "+START+(" moves "+" ".join(moves[:k]) if k else "")); send(f"go depth {depth}"); t=time.time(); until("bestmove"); dt=time.time()-t; ts.append(dt)
print(" ".join(f"{x:.1f}" for x in ts)); print(f"total {sum(ts):.0f}s over {len(ts)} positions")
send("quit")
