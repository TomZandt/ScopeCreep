"""The fixed set of test clips and how to (re)generate them into the test-data directory."""
import os
import sys

from . import media, paths

HD = (1920, 1080)
UHD = (3840, 2160)
FPS = 24
HD_FRAMES = 24      # one second
UHD_UNIQUE = 24     # unique UHD frames, looped by symlink up to UHD_FRAMES
UHD_FRAMES = 120    # five seconds

# name -> (width, height, frames, unique, pixel_type, frame factory)
CLIPS = {
    "ramp": (*HD, HD_FRAMES, 1, "float", lambda w, h: (lambda f: media.ramp(w, h))),
    "chart": (*HD, HD_FRAMES, 1, "float", lambda w, h: (lambda f: media.chart(w, h))),
    "outofrange": (*HD, HD_FRAMES, 1, "float", lambda w, h: (lambda f: media.out_of_range(w, h))),
    "natural": (*HD, HD_FRAMES, HD_FRAMES, "float", lambda w, h: (lambda f: media.natural(w, h, f))),
    "uhd": (*UHD, UHD_FRAMES, UHD_UNIQUE, "half", lambda w, h: (lambda f: media.natural(w, h, f))),
}


def clip_dir(name):
    return os.path.join(paths.MEDIA, name)


def first_frame(name):
    return os.path.join(clip_dir(name), "%s.0001.exr" % name)


def is_complete(name):
    w, h, frames, *_ = CLIPS[name]
    d = clip_dir(name)
    return all(os.path.lexists(os.path.join(d, "%s.%04d.exr" % (name, i + 1))) for i in range(frames))


def generate(names=None, force=False):
    for name in names or CLIPS:
        w, h, frames, unique, ptype, factory = CLIPS[name]
        if is_complete(name) and not force:
            print("media: %s already present (use --force to regenerate)" % name)
            continue
        print("media: generating %s (%dx%d, %d frames, %s)" % (name, w, h, frames, ptype))
        sys.stdout.flush()
        media.write_sequence(clip_dir(name), name, factory(w, h), frames, unique=unique, pixel_type=ptype)
