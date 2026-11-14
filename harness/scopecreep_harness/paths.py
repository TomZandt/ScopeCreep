"""Where generated data lives. Never inside the git repo."""
import os

ROOT = os.environ.get("SCOPECREEP_TESTDATA") or os.path.expanduser("~/scopecreep-testdata")
MEDIA = os.path.join(ROOT, "media")
RENDERS = os.path.join(ROOT, "renders")
DUMPS = os.path.join(ROOT, "dumps")
SHOTS = os.path.join(ROOT, "screenshots")

PROJECT_PREFIX = "scopecreep-test-"
