import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import setups, paths


class SetupCatalogue(unittest.TestCase):
    def test_every_project_name_is_prefixed_and_unique(self):
        names = [s.project_name for s in setups.SETUPS]
        self.assertEqual(len(names), len(set(names)))
        self.assertTrue(all(n.startswith(paths.PROJECT_PREFIX) for n in names))

    def test_colour_science_is_set_before_the_spaces_that_depend_on_it(self):
        for s in setups.SETUPS:
            keys = [k for k, _ in s.settings]
            self.assertEqual(keys[0], "colorScienceMode", s.project_name)

    def test_covers_the_issue_matrix(self):
        by = {s.key: dict(s.settings) for s in setups.SETUPS}
        modes = {v["colorScienceMode"] for v in by.values()}
        self.assertEqual(modes, {"davinciYRGB", "davinciYRGBColorManagedv2", "acescct"})
        outs = {(v["colorScienceMode"], v.get("colorSpaceOutput") or v.get("colorAcesODT")) for v in by.values()}
        # DWG/DI to Rec.709, P3-D65 and Rec.2100 PQ; ACEScct to the same three
        self.assertGreaterEqual(len(outs), 7)
        pq = [v for k, v in by.items() if k.startswith("dwg-pq")]
        self.assertEqual({v["colorSpaceOutputToneMapping"] for v in pq}, {"None", "DaVinci"})

    def test_perf_project_is_uhd(self):
        self.assertEqual(setups.PERF.resolution, (3840, 2160))


if __name__ == "__main__":
    unittest.main()
