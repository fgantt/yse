# Aggregate immediate callers of functions matching a pattern in a macOS `sample` call graph.
import re,sys,collections
path,pat=sys.argv[1],sys.argv[2]; levels=int(sys.argv[3]) if len(sys.argv)>3 else 1
lines=open(path).read().split("Call graph:")[1].split("Total number in stack")[0].splitlines()
stack=[]; agg=collections.Counter()
for l in lines:
    m=re.match(r"^([\s+!:|]*)(\d+)\s+(.*?)\s+\(in ",l)
    if not m: continue
    depth=len(m.group(1)); cnt=int(m.group(2)); name=re.sub(r"::h[0-9a-f]{16}","",m.group(3))
    while stack and stack[-1][0]>=depth: stack.pop()
    if re.search(pat,name):
        chain=" <- ".join(s[1][-70:] for s in reversed(stack[-levels:]))
        agg[chain]+=cnt
    stack.append((depth,name))
for k,v in agg.most_common(12): print(f"{v:6d}  {k}")
