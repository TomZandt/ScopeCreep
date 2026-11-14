"""The Colour Setups the harness builds, one Resolve project each.

Values are the exact strings Resolve 21.2 Studio accepts through Project.SetSetting (verified by read-back;
Resolve returns False for values it does not recognise).
"""
from dataclasses import dataclass, field

from .paths import PROJECT_PREFIX

REC709 = "Rec.709 Gamma 2.4"
DWG = "DaVinci WG/Intermediate"
LINEAR_AP1 = "ACEScg"  # scene-linear wide gamut; Resolve has no linear DaVinci WG input option
P3D65 = "P3-D65"
PQ = "Rec.2100 ST2084"


@dataclass
class Setup:
    key: str
    title: str
    settings: list  # ordered (key, value) pairs; colorScienceMode always first
    resolution: tuple = (1920, 1080)
    fps: str = "24"
    clips: tuple = ("ramp", "chart", "outofrange", "natural")
    notes: str = ""
    # Clip "Input Color Space" to tag the test media with, so the Output Conversion does real work instead of
    # round-tripping Resolve's default input space. None = leave Resolve's default.
    input_space: str = None
    project_name: str = field(init=False)

    def __post_init__(self):
        self.project_name = PROJECT_PREFIX + self.key


def _yrgb(key, title, tl, out):
    return Setup(key, title, [
        ("colorScienceMode", "davinciYRGB"),
        ("colorSpaceTimeline", tl),
        ("colorSpaceOutput", out),
    ])


def _rcm(key, title, out, tonemap="None"):
    return Setup(key, title, [
        ("colorScienceMode", "davinciYRGBColorManagedv2"),
        ("colorSpaceTimeline", DWG),
        ("colorSpaceOutput", out),
        ("colorSpaceOutputToneMapping", tonemap),
        ("colorSpaceOutputGamutMapping", "None"),
    ], input_space=LINEAR_AP1)


def _aces(key, title, odt):
    return Setup(key, title, [
        ("colorScienceMode", "acescct"),
        ("colorAcesODT", odt),
    ], notes="ACES version is not settable through the scripting API (see report)")


SETUPS = [
    _yrgb("yrgb-rec709g24", "DaVinci YRGB, Rec.709 Gamma 2.4 timeline and output", REC709, REC709),
    _rcm("dwg-rec709", "Colour Managed, DWG/DI timeline, Rec.709 Gamma 2.4 output", REC709),
    _rcm("dwg-p3d65", "Colour Managed, DWG/DI timeline, P3-D65 output", P3D65),
    _rcm("dwg-pq-tmoff", "Colour Managed, DWG/DI timeline, Rec.2100 PQ output, tone mapping off", PQ, "None"),
    _rcm("dwg-pq-tmon", "Colour Managed, DWG/DI timeline, Rec.2100 PQ output, tone mapping on", PQ, "DaVinci"),
    _aces("acescct-rec709", "ACEScct, Rec.709 output transform", "Rec.709 BT.1886"),
    _aces("acescct-p3d65", "ACEScct, P3-D65 output transform", "P3-D65"),
    _aces("acescct-pq", "ACEScct, Rec.2100 PQ (1000 nit) output transform", "Rec.2100 ST2084 (1000 nit)"),
]

PERF = Setup("perf-uhd", "UHD 24p performance project (YRGB, Rec.709 Gamma 2.4)", [
    ("colorScienceMode", "davinciYRGB"),
    ("colorSpaceTimeline", REC709),
    ("colorSpaceOutput", REC709),
], resolution=(3840, 2160), clips=("uhd",))

ALL = SETUPS + [PERF]


def by_key(key):
    for s in ALL:
        if s.key == key:
            return s
    raise KeyError(key)
