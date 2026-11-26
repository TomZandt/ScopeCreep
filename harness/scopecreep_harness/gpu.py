"""GPU load while a check runs, from `nvidia-smi` (shipped with NVIDIA's driver on Linux and Windows).

`start` samples once a second into a CSV until the returned process is terminated; `summary` reads it back."""
import subprocess

FIELDS = ("timestamp,utilization.gpu,clocks.sm,power.draw,memory.used,"
          "pcie.link.gen.current,pcie.link.width.current")


def start(path, program="nvidia-smi", every_ms=1000):
    """Start sampling into `path`. The process to terminate when done, or None without `nvidia-smi`."""
    with open(path, "w") as out:  # the child keeps its own handle
        try:
            return subprocess.Popen([program, "--query-gpu=" + FIELDS, "--format=csv,noheader,nounits",
                                     "-lms", str(every_ms)], stdout=out, stderr=subprocess.DEVNULL)
        except OSError:
            return None


def stop(proc):
    if proc is not None:
        proc.terminate()
        try:
            proc.wait(5)
        except subprocess.TimeoutExpired:
            proc.kill()


def _number(text):
    try:
        return float(text)
    except ValueError:
        return None  # [N/A], [Not Supported]


def summary(path):
    """One line: mean and peak GPU utilisation and the most memory used."""
    util, memory = [], []
    try:
        with open(path) as f:
            for line in f:
                cells = [c.strip() for c in line.split(",")]
                if len(cells) < 5:
                    continue
                u, m = _number(cells[1]), _number(cells[4])
                if u is not None:
                    util.append(u)
                if m is not None:
                    memory.append(m)
    except OSError:
        pass
    if not util:
        return "GPU: no samples"
    return "GPU %.0f%% mean, %.0f%% max, %s MiB max over %d samples" % (
        sum(util) / len(util), max(util), "%.0f" % max(memory) if memory else "n/a", len(util))
