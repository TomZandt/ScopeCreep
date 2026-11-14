"""Read the Colour Setup (and related facts) out of Resolve by script, and save it.

This is Stage 0 check 2: everything here comes from the scripting API only.
"""
import json
import os

from . import paths, setups

# Colour Setup keys, per the glossary: colour science mode, Timeline/Output Colour Space, tone and gamut
# mapping, ACES version and output transform, Levels. Missing keys are recorded as None, never guessed.
COLOUR_KEYS = [
    "colorScienceMode", "isAutoColorManage", "rcmPresetMode", "separateColorSpaceAndGamma",
    "colorSpaceTimeline", "colorSpaceTimelineGamma", "colorSpaceInput", "colorSpaceInputGamma",
    "colorSpaceOutput", "colorSpaceOutputGamma",
    "colorSpaceOutputToneMapping", "colorSpaceOutputGamutMapping", "colorSpaceOutputToneLuminanceMax",
    "colorSpaceOutputGamutSaturationKnee", "colorSpaceOutputGamutSaturationMax", "colorSpaceOutputGamutLimit",
    "inputDRT", "outputDRT", "useInverseDRT", "inputDRTSatRolloffStart", "inputDRTSatRolloffLimit",
    "outputDRTSatRolloffStart", "outputDRTSatRolloffLimit",
    "timelineWorkingLuminanceMode", "timelineWorkingLuminance", "graphicsWhiteLevel", "use203NitsReference",
    "acesVersion", "colorAcesIDT", "colorAcesODT", "colorAcesGamutCompressType", "colorAcesMidGray",
    "colorAcesNodeLUTProcessingSpace",
    "videoDataLevels", "videoDataLevelsRetainSubblockAndSuperWhiteData", "useColorSpaceAwareGradingTools",
    "useCATransform", "disableFusionToneMapping", "imageResizingGamma",
    "hdrMasteringOn", "hdrMasteringLuminanceMax",
]
TIMING_KEYS = ["timelineFrameRate", "timelinePlaybackFrameRate", "timelineDropFrameTimecode",
               "timelineResolutionWidth", "timelineResolutionHeight",
               "timelineOutputResolutionWidth", "timelineOutputResolutionHeight"]


def _subset(d, keys):
    return {k: d.get(k) for k in keys}


def _graph(graph):
    if graph is None:
        return None
    n = graph.GetNumNodes()
    return {
        "numNodes": n,
        "nodes": [{
            "index": i,
            "label": graph.GetNodeLabel(i),
            "tools": graph.GetToolsInNode(i),
            "cacheMode": graph.GetNodeCacheMode(i),
        } for i in range(1, n + 1)],
    }


def dump_project(resolve, project):
    timeline = project.GetCurrentTimeline()
    proj = project.GetSetting()
    tl = timeline.GetSetting() if timeline else {}
    item = timeline.GetItemListInTrack("video", 1)[0] if timeline else None
    out = {
        "product": {
            "name": resolve.GetProductName(),
            "version": resolve.GetVersionString(),
            "isStudio": resolve.IsStudio(),
        },
        "project": project.GetName(),
        "timeline": timeline.GetName() if timeline else None,
        "colourSetup": {
            "project": _subset(proj, COLOUR_KEYS),
            "timeline": _subset(tl, COLOUR_KEYS),
            "timelineUsesProjectSettings": tl.get("useCustomSettings"),
        },
        "playback": {
            "project": _subset(proj, TIMING_KEYS),
            "timeline": _subset(tl, TIMING_KEYS),
        },
        "cacheModeConstants": {"CACHE_AUTO_ENABLED": resolve.CACHE_AUTO_ENABLED,
                               "CACHE_DISABLED": resolve.CACHE_DISABLED,
                               "CACHE_ENABLED": resolve.CACHE_ENABLED},
        "timelineNodeTree": _graph(timeline.GetNodeGraph()) if timeline else None,
        "firstClipNodeTree": _graph(item.GetNodeGraph()) if item else None,
        "allProjectSettings": proj,
        "allTimelineSettings": tl,
    }
    return out


def save(name, data):
    os.makedirs(paths.DUMPS, exist_ok=True)
    p = os.path.join(paths.DUMPS, name + ".json")
    with open(p, "w") as f:
        json.dump(data, f, indent=1, sort_keys=True)
    return p


def dump_all(resolve, selected=None):
    from . import projects
    pm = resolve.GetProjectManager()
    original = projects.remember_original(pm)
    summaries = []
    for s in setups.ALL:
        if selected and s.key not in selected:
            continue
        project = projects.load(pm, s.project_name)
        if not project:
            print("dump: %s not found (run `harness projects`)" % s.project_name)
            continue
        data = dump_project(resolve, project)
        p = save(s.project_name, data)
        summaries.append((s, data))
        print("dump: wrote", p)
    if summaries:
        md = markdown_summary(summaries)
        mp = os.path.join(paths.DUMPS, "colour-setup-summary.md")
        with open(mp, "w") as f:
            f.write(md)
        print("dump: wrote", mp)
    if original:
        projects.load(pm, original)
    return summaries


SUMMARY_KEYS = ["colorScienceMode", "colorSpaceTimeline", "colorSpaceTimelineGamma", "colorSpaceOutput",
                "colorSpaceOutputGamma", "colorSpaceOutputToneMapping", "colorSpaceOutputGamutMapping",
                "outputDRT", "inputDRT", "colorAcesIDT", "colorAcesODT", "acesVersion",
                "colorAcesGamutCompressType", "videoDataLevels", "isAutoColorManage"]


def markdown_summary(summaries):
    lines = []
    prod = summaries[0][1]["product"]
    lines.append("Resolve: %s %s, IsStudio=%s" % (prod["name"], prod["version"], prod["isStudio"]))
    lines.append("Node cache-mode constants: %r\n" % summaries[0][1]["cacheModeConstants"])
    for s, d in summaries:
        lines.append("### %s\n" % s.project_name)
        lines.append("%s\n" % s.title)
        lines.append("| key | project | timeline |\n|---|---|---|")
        cs = d["colourSetup"]
        for k in SUMMARY_KEYS:
            lines.append("| `%s` | `%r` | `%r` |" % (k, cs["project"].get(k), cs["timeline"].get(k)))
        pb = d["playback"]
        lines.append("\nPlayback frame rate: `timelinePlaybackFrameRate=%r`, `timelineFrameRate=%r`\n"
                     % (pb["project"].get("timelinePlaybackFrameRate"), pb["project"].get("timelineFrameRate")))
        for label, tree in (("Timeline node tree", d["timelineNodeTree"]), ("First clip node tree", d["firstClipNodeTree"])):
            if tree is None:
                lines.append("%s: unavailable\n" % label)
                continue
            lines.append("%s: %d node(s)\n" % (label, tree["numNodes"]))
            for n in tree["nodes"]:
                lines.append("- node %d label=%r tools=%r cacheMode=%r" % (n["index"], n["label"], n["tools"], n["cacheMode"]))
            lines.append("")
    return "\n".join(lines)
