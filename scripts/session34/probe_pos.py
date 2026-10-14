# Score one game position with YaneuraOu at several node counts and ours at several times.
import subprocess,sys,threading,queue,os
REPO="/Users/fgantt/projects/vibe/shogi-game/yse-worktrees/nnue"
YANE="/Users/fgantt/projects/vibe/shogi-game/yaneuraou-V900-git-mac-all/NNUE_halfkpvm_256x2_32_32/YaneuraOu_NNUE_halfkpvm_256x2_32_32-V900Git_APPLEM1"
game,ply=sys.argv[1],int(sys.argv[2])
lines=[l for l in open(game) if l.strip()]
start=[l for l in lines if l.startswith("# start:")][0].split("start:")[1].strip().replace("sfen ","")
moves=[l for l in lines if not l.startswith("#")][0].split()
pos="sfen "+start+" moves "+" ".join(moves[:ply-1])
class E:
    def __init__(s,path,cwd,opts):
        s.p=subprocess.Popen([path],cwd=cwd,stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,text=True,bufsize=1,env={**os.environ,"SHOGI_PREFS_DIR":REPO+"/matches/s34_threads/prefs"})
        s.q=queue.Queue(); threading.Thread(target=lambda:[s.q.put(l) for l in s.p.stdout],daemon=True).start()
        s.send("usi"); [s.send(f"setoption name {k} value {v}") for k,v in opts]; s.send("isready"); s.until("readyok")
    def send(s,c): s.p.stdin.write(c+"\n"); s.p.stdin.flush()
    def until(s,t):
        out=[]
        while True:
            l=s.q.get(timeout=600); out.append(l)
            if l.startswith(t): return out
    def go(s,go):
        s.send("usinewgame"); s.send("position "+pos); s.send("go "+go); out=s.until("bestmove"); last=""
        for l in out:
            if l.startswith("info depth") and "score" in l and "bound" not in l: last=l
        t=last.split(); g=lambda k: t[t.index(k)+1] if k in t else "?"
        sc=" ".join(t[t.index("score")+1:t.index("score")+3]) if "score" in t else "?"
        return f"depth {g('depth')} {sc} best {out[-1].split()[1]} pv {' '.join(t[t.index('pv')+1:t.index('pv')+7]) if 'pv' in t else ''}"
y=E(YANE,os.path.dirname(YANE),[("USI_OwnBook","false"),("BookFile","no_book"),("Threads","1"),("USI_Hash","256")])
for n in [int(x) for x in sys.argv[3].split(",")]: print(f"yane nodes {n:>9}:",y.go(f"nodes {n}"),flush=True)
o=E(REPO+"/engines/usi-engine-stable",REPO,[("NNUEFile","nnue/default.bin"),("USI_Threads","16")])
for ms in [int(x) for x in sys.argv[4].split(",")]: print(f"ours {ms/1000:>5.0f} s:",o.go(f"btime 0 wtime 0 byoyomi {ms}"),flush=True)
