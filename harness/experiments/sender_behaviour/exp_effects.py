import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from lib import *
r, pm = connect()
cats = {}
for e in r.GetEffectsList("video"):
    cats.setdefault(e.get("category"), []).append(e.get("name"))
for c, v in cats.items():
    print(c, len(v), [n for n in v if "djust" in n or "cope" in n or "Adj" in n])
print(r.GetEffectsList("video")[:3])
