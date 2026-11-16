import json
import os
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import dump, paths, setups


def fake_dump(output):
    return {
        "product": {"name": "DaVinci Resolve", "version": "21.2", "isStudio": True},
        "cacheModeConstants": {},
        "colourSetup": {"project": {"colorSpaceOutput": output}, "timeline": {"colorSpaceOutput": output}},
        "playback": {"project": {}, "timeline": {}},
        "timelineNodeTree": None,
        "firstClipNodeTree": None,
    }


class Summary(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.dir.cleanup)
        patcher = mock.patch.object(paths, "DUMPS", self.dir.name)
        patcher.start()
        self.addCleanup(patcher.stop)

    def summary(self):
        with open(os.path.join(self.dir.name, "colour-setup-summary.md")) as f:
            return f.read()

    def test_a_partial_dump_keeps_the_other_projects_in_the_summary(self):
        a, b = setups.by_key("dwg-rec709"), setups.by_key("acescct-pq")
        dump.save(a.project_name, fake_dump("Rec.709 Gamma 2.4"))
        dump.save(b.project_name, fake_dump("old"))

        dump.write_summary([(b, fake_dump("Rec.2100 ST2084"))])

        md = self.summary()
        self.assertIn("### scopecreep-test-dwg-rec709", md)
        self.assertIn("'Rec.709 Gamma 2.4'", md)
        self.assertIn("### scopecreep-test-acescct-pq", md)
        self.assertIn("'Rec.2100 ST2084'", md)
        self.assertNotIn("'old'", md)

    def test_projects_appear_in_setup_order(self):
        first, later = setups.ALL[0], setups.ALL[-1]
        dump.save(later.project_name, fake_dump("x"))
        dump.write_summary([(first, fake_dump("y"))])
        md = self.summary()
        self.assertLess(md.index("### " + first.project_name), md.index("### " + later.project_name))

    def test_a_dump_on_disk_that_is_not_json_is_skipped(self):
        with open(os.path.join(self.dir.name, setups.ALL[1].project_name + ".json"), "w") as f:
            f.write("{")
        dump.write_summary([(setups.ALL[0], fake_dump("y"))])
        self.assertNotIn(setups.ALL[1].project_name, self.summary())


if __name__ == "__main__":
    unittest.main()
