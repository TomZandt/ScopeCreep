import os, sys; sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
# usage: key.py [MODS+]KEY [more ...] [--shot path]   e.g. key.py ALT+P Down Down
from lib import *
args = sys.argv[1:]
shot = None
if "--shot" in args:
    i = args.index("--shot"); shot = args[i + 1]; args = args[:i] + args[i + 2:]
for a in args:
    parts = a.split("+")
    viewer.send_key(parts[-1], " ".join(parts[:-1]) if len(parts) > 1 else "")
    time.sleep(0.4)
if shot:
    time.sleep(0.8)
    print(viewer.screenshot(shot))
