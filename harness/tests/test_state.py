import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import state

EMPTY = {"numNodes": 1, "nodes": [{"index": 1, "label": "", "tools": [], "cacheMode": -1}]}
GRADED = {"numNodes": 1, "nodes": [{"index": 1, "label": "", "tools": ["Primary Balance", "Saturation, Hue & Lum Mix"],
                                    "cacheMode": -1}]}
SENDER = {"numNodes": 1, "nodes": [{"index": 1, "label": "sender",
                                    "tools": ["OFX: scopecreep Sender (standing in for the probe)"], "cacheMode": None}]}
SETTINGS = {"colorScienceMode": "davinciYRGBColorManagedv2", "colorSpaceOutput": "Rec.709", "unrelated": "x"}


def text(clip_graphs, settings=SETTINGS, timeline_graph=SENDER):
    return state.describe("scopecreep-test-dwg-rec709", "scopecreep-dwg-rec709", settings, timeline_graph, clip_graphs)


class Describe(unittest.TestCase):
    def test_an_ungraded_clip_reads_no_grade(self):
        self.assertIn("clip ramp grade = none", text([("ramp", EMPTY)]).splitlines())

    def test_a_graded_clip_names_its_tools(self):
        self.assertIn("clip ramp grade = node 1: Primary Balance, Saturation, Hue & Lum Mix",
                      text([("ramp", GRADED)]).splitlines())

    def test_the_sender_on_the_timeline_is_named_whatever_its_build(self):
        self.assertIn("timeline grade = node 1: Sender", text([]).splitlines())

    def test_only_colour_setup_keys_are_recorded(self):
        lines = text([]).splitlines()
        self.assertIn("colour colorScienceMode = davinciYRGBColorManagedv2", lines)
        self.assertFalse(any("unrelated" in l for l in lines))


class Differences(unittest.TestCase):
    def test_the_same_state_has_no_differences(self):
        self.assertEqual(state.differences(text([("ramp", EMPTY)]), text([("ramp", EMPTY)])), [])

    def test_a_grade_added_since_is_named(self):
        got = state.differences(text([("ramp", EMPTY)]), text([("ramp", GRADED)]))
        self.assertEqual(got, ["clip ramp grade: recorded none, now node 1: Primary Balance, Saturation, Hue & Lum Mix"])

    def test_a_missing_line_is_a_difference(self):
        got = state.differences(text([("ramp", EMPTY), ("chart", EMPTY)]), text([("ramp", EMPTY)]))
        self.assertEqual(got, ["clip chart grade: recorded none, now (absent)"])


class Grades(unittest.TestCase):
    def test_lists_only_the_graded_clips(self):
        self.assertEqual(state.graded_clips(text([("ramp", GRADED), ("chart", EMPTY)])),
                         ["ramp: node 1: Primary Balance, Saturation, Hue & Lum Mix"])


class Problems(unittest.TestCase):
    def test_a_project_that_matches_its_ungraded_references_has_none(self):
        self.assertEqual(state.problems(text([("ramp", EMPTY)]), text([("ramp", EMPTY)])), [])

    def test_references_without_a_recorded_state_are_a_problem(self):
        self.assertEqual(len(state.problems(None, text([("ramp", EMPTY)]))), 1)

    def test_a_grade_is_a_problem_even_when_the_references_have_it_too(self):
        got = state.problems(text([("ramp", GRADED)]), text([("ramp", GRADED)]))
        self.assertEqual(got, ["clip ramp carries a grade (test projects carry none): "
                               "node 1: Primary Balance, Saturation, Hue & Lum Mix"])


class ClipKey(unittest.TestCase):
    def test_an_image_sequence_item_is_its_clip(self):
        self.assertEqual(state.clip_key("outofrange.[0001-0024].exr"), "outofrange")


if __name__ == "__main__":
    unittest.main()
