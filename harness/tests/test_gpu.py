import os
import sys
import tempfile
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

from scopecreep_harness import gpu

# As `nvidia-smi --query-gpu=... --format=csv,noheader,nounits` writes it, one GPU, one line per sample.
SAMPLES = """2026/11/26 10:00:00.100, 40, 1900, 250.50, 9000, 4, 16
2026/11/26 10:00:01.100, 60, 1950, [N/A], 9100, 4, 16
"""


class Summary(unittest.TestCase):
    def write(self, text):
        f = tempfile.NamedTemporaryFile("w", suffix=".csv", delete=False)
        f.write(text)
        f.close()
        self.addCleanup(os.remove, f.name)
        return f.name

    def test_averages_utilisation_and_takes_the_most_memory(self):
        self.assertEqual(gpu.summary(self.write(SAMPLES)), "GPU 50% mean, 60% max, 9100 MiB max over 2 samples")

    def test_no_samples(self):
        self.assertEqual(gpu.summary(self.write("")), "GPU: no samples")
        self.assertEqual(gpu.summary("/nonexistent/gpu.csv"), "GPU: no samples")

    def test_a_machine_without_nvidia_smi_gets_no_monitor(self):
        self.assertIsNone(gpu.start(self.write(""), program="/nonexistent/nvidia-smi"))


if __name__ == "__main__":
    unittest.main()
