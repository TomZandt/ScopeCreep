#!/usr/bin/env python3
"""PROTOTYPE (throwaway): list / apply the scopecreep probe Sender by script.

  apply_probe.py            list OFX effects matching "scopecreep" (read-only)
  apply_probe.py --apply    also AddEffect('ofx', <id>) to the first clip on
                            video track 1 of the current timeline (modifies project)

Needs Resolve Studio running with external scripting set to Local.
"""
import os
import sys

os.environ.setdefault("RESOLVE_SCRIPT_API", "/opt/resolve/Developer/Scripting")
os.environ.setdefault("RESOLVE_SCRIPT_LIB", "/opt/resolve/libs/Fusion/fusionscript.so")
sys.path.insert(0, os.path.join(os.environ["RESOLVE_SCRIPT_API"], "Modules"))
import DaVinciResolveScript as dvr  # noqa: E402

resolve = dvr.scriptapp("Resolve")
if resolve is None:
    sys.exit("Cannot connect to Resolve (running? Preferences > System > General > External scripting = Local)")
print("Resolve", resolve.GetVersionString(), "| product:", resolve.GetProductName())

effects = [e for e in resolve.GetEffectsList("video") if e.get("category") == "ofx"]
print(f"{len(effects)} OFX video effects listed")
hits = [e for e in effects if "scopecreep" in (e.get("id", "") + e.get("name", "")).lower()]
for e in hits:
    print("FOUND:", e)
if not hits:
    sys.exit("scopecreep probe NOT listed (installed in /usr/OFX/Plugins? Resolve restarted since?)")

if "--apply" in sys.argv:
    tl = resolve.GetProjectManager().GetCurrentProject().GetCurrentTimeline()
    item = tl.GetItemListInTrack("video", 1)[0]
    eff = item.AddEffect("ofx", hits[0]["id"])
    print("AddEffect ->", eff, "| effects on item:", item.GetEffects("ofx"))
