import os
import sys
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
from scopecreep_harness import exr
a = exr.read_exr(sys.argv[1]); b = exr.read_exr(sys.argv[2])
assert (a.width, a.height) == (b.width, b.height), (a.width, a.height, b.width, b.height)
mx = 0.0; sm = 0.0; n = 0; at = None
for y, (ra, rb) in enumerate(zip(a.rows, b.rows)):
    for c in range(3):
        for x, (u, v) in enumerate(zip(ra[c], rb[c])):
            d = abs(u - v)
            sm += d; n += 1
            if d > mx: mx = d; at = (x, y, c, u, v)
print("max %.3e mean %.3e at(x,y,ch,a,b)=%s samples=%d" % (mx, sm / n, at, n))
